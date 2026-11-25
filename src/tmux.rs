use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdout, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use crate::model::{
    CurrentWindow, Field, FieldIssue, InvokingContext, IssueCategory, Session, SessionId,
    SessionSnapshot, WindowId, decode_name, escape_text, positive_field,
};

#[derive(Debug)]
pub enum ProcessError {
    Io(io::Error),
    Cleanup(io::Error),
    Timeout,
    Cancelled,
    ResourceExhaustion,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(
                formatter,
                "tmux subprocess failed: {error}; check that tmux is installed and accessible"
            ),
            Self::Cleanup(error) => write!(formatter, "tmux subprocess cleanup failed: {error}"),
            Self::Timeout => write!(formatter, "tmux request timed out"),
            Self::Cancelled => write!(formatter, "tmux request cancelled"),
            Self::ResourceExhaustion => write!(formatter, "insufficient memory for tmux output"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<io::Error> for ProcessError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn verify_version(value: &str, component: &str) -> Result<(), String> {
    let version = value.trim().strip_prefix("tmux ").unwrap_or(value.trim());
    let Some((major, minor)) = version.split_once('.') else {
        return Err(format!("cannot verify tmux {component} version"));
    };
    let minor_digits = minor.bytes().take_while(u8::is_ascii_digit).count();
    let (minor, suffix) = minor.split_at(minor_digits);
    let prerelease_suffix = suffix.strip_prefix('-').unwrap_or(suffix);
    let prerelease = ["rc", "alpha", "beta", "pre", "preview", "dev"]
        .iter()
        .any(|prefix| {
            prerelease_suffix
                .strip_prefix(prefix)
                .is_some_and(|number| number.bytes().all(|byte| byte.is_ascii_digit()))
        });
    if major.is_empty()
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || minor.is_empty()
        || !(prerelease
            || suffix.is_empty()
            || (suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_lowercase())))
    {
        return Err(format!("cannot verify tmux {component} version"));
    }
    let version = major
        .parse::<u64>()
        .and_then(|major| minor.parse::<u64>().map(|minor| (major, minor)))
        .map_err(|_| format!("cannot verify tmux {component} version"))?;
    if prerelease || version < (3, 3) {
        return Err(format!(
            "unsupported tmux {component} version; upgrade to tmux 3.3 or later"
        ));
    }
    Ok(())
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}

impl OwnedChild {
    fn stop(&mut self) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        // The child is the leader of its own process group; it has not been reaped.
        let killed = unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
        let group_error = if killed == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                None
            } else {
                Some(error)
            }
        } else {
            None
        };
        if group_error.is_some() {
            self.child.kill()?;
        }
        self.child.wait()?;
        self.reaped = true;
        if let Some(error) = group_error {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("failed to stop tmux subprocess: {error}");
        }
    }
}

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn drain(
    pipe: &mut impl Read,
    destination: &mut Vec<u8>,
    eof: &mut bool,
) -> Result<(), ProcessError> {
    let mut buffer = [0; 8192];
    for _ in 0..16 {
        if *eof {
            break;
        }
        match pipe.read(&mut buffer) {
            Ok(0) => *eof = true,
            Ok(length) => {
                destination
                    .try_reserve(length)
                    .map_err(|_| ProcessError::ResourceExhaustion)?;
                destination.extend_from_slice(&buffer[..length]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn collect(
    child: &mut OwnedChild,
    mut stdout: ChildStdout,
    mut stderr: ChildStderr,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Output, ProcessError> {
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut out_eof = false;
    let mut err_eof = false;
    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProcessError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ProcessError::Timeout);
        }
        drain(&mut stdout, &mut out, &mut out_eof)?;
        drain(&mut stderr, &mut err, &mut err_eof)?;
        if Instant::now() >= deadline {
            return Err(ProcessError::Timeout);
        }
        // Keep the leader unreaped while a descendant might still hold either pipe.
        if out_eof
            && err_eof
            && let Some(status) = child.child.try_wait()?
        {
            child.reaped = true;
            return Ok(Output {
                status,
                stdout: out,
                stderr: err,
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn run_process(
    mut command: Command,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Output, ProcessError> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(ProcessError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ProcessError::Timeout);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = loop {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ProcessError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ProcessError::Timeout);
        }
        match command.spawn() {
            Ok(child) => break child,
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut child = OwnedChild {
        child,
        reaped: false,
    };
    let stdout = child.child.stdout.take().ok_or_else(|| {
        ProcessError::Io(io::Error::other("tmux subprocess stdout was not captured"))
    })?;
    let stderr = child.child.stderr.take().ok_or_else(|| {
        ProcessError::Io(io::Error::other("tmux subprocess stderr was not captured"))
    })?;
    let result = collect(&mut child, stdout, stderr, deadline, cancelled);
    if result.is_err() {
        child.stop().map_err(ProcessError::Cleanup)?;
    }
    result
}

#[derive(Debug)]
pub enum AcquireError {
    Process(ProcessError),
    Invalid(String),
}

impl fmt::Display for AcquireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => write!(formatter, "{error}"),
            Self::Invalid(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for AcquireError {}

impl From<ProcessError> for AcquireError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<String> for AcquireError {
    fn from(error: String) -> Self {
        Self::Invalid(error)
    }
}

fn check_request(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ProcessError> {
    if cancelled.load(Ordering::SeqCst) {
        Err(ProcessError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(ProcessError::Timeout)
    } else {
        Ok(())
    }
}

pub struct TmuxClient {
    executable: OsString,
    context: Option<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl TmuxClient {
    pub fn new(executable: impl Into<OsString>, context: Option<OsString>) -> Self {
        Self {
            executable: executable.into(),
            context,
            environment: Vec::new(),
        }
    }

    pub fn from_environment() -> Self {
        Self::new("tmux", std::env::var_os("TMUX"))
    }

    pub fn env(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) {
        self.environment.push((key.into(), value.into()));
    }

    fn command(&self, socket: Option<&OsStr>, args: &[&OsStr]) -> Command {
        let mut command = Command::new(&self.executable);
        command.env_remove("TMUX");
        command.envs(self.environment.iter().map(|(key, value)| (key, value)));
        command.env("LC_ALL", "C");
        command.arg("-u");
        if let Some(socket) = socket {
            command.arg("-S").arg(socket);
        }
        command.args(args);
        command
    }

    fn request(
        &self,
        socket: Option<&OsStr>,
        args: &[&str],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Output, AcquireError> {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        Ok(run_process(
            self.command(socket, &args),
            deadline,
            cancelled,
        )?)
    }

    pub fn acquire(&self, cancelled: &AtomicBool) -> Result<SessionSnapshot, AcquireError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let socket = self.context.as_deref().map(parse_context).transpose()?;
        let client = self.request(None, &["-V"], deadline, cancelled)?;
        let version = successful_line(&client, "client version")?;
        verify_version(version, "client").map_err(AcquireError::Invalid)?;

        let probe = self.request(
            socket.as_deref(),
            &["display-message", "-p", "#{version}"],
            deadline,
            cancelled,
        )?;
        if !probe.status.success() && socket.is_none() && absent_server(&probe.stderr) {
            return finish_snapshot(
                Vec::new(),
                InvokingContext::OutsideTmux,
                deadline,
                cancelled,
            );
        }
        let version = successful_line(&probe, "server version")?;
        verify_version(version, "server").map_err(AcquireError::Invalid)?;

        loop {
            check_request(deadline, cancelled)?;
            let rows = self.list(socket.as_deref(), deadline, cancelled)?;
            let mut sessions = Vec::new();
            sessions
                .try_reserve(rows.len())
                .map_err(|_| ProcessError::ResourceExhaustion)?;
            let mut retry = false;
            for mut session in rows {
                check_request(deadline, cancelled)?;
                let name = self.request(
                    socket.as_deref(),
                    &[
                        "display-message",
                        "-p",
                        "-t",
                        session.id.as_str(),
                        "#{session_name}",
                    ],
                    deadline,
                    cancelled,
                )?;
                if !name.status.success() {
                    let current = self.list(socket.as_deref(), deadline, cancelled)?;
                    check_request(deadline, cancelled)?;
                    let mut still_exists = false;
                    for row in &current {
                        check_request(deadline, cancelled)?;
                        if row.id == session.id {
                            still_exists = true;
                            break;
                        }
                    }
                    if !still_exists {
                        retry = true;
                        break;
                    }
                    session.name = Field::Unavailable(FieldIssue {
                        session_id: None,
                        field: "name".to_owned(),
                        category: IssueCategory::Connection,
                        message: command_error("session name", &name.stderr).to_string(),
                        recovery: Some("Check tmux and retry.".to_owned()),
                    });
                    sessions.push(session);
                    continue;
                }
                let Some(name) = name.stdout.strip_suffix(b"\n") else {
                    return Err(AcquireError::Invalid(
                        "malformed tmux session-name response".to_owned(),
                    ));
                };
                session.name = decode_name(name, "name");
                sessions.push(session);
            }
            if retry {
                continue;
            }
            let current = self.list(socket.as_deref(), deadline, cancelled)?;
            if sessions.len() != current.len() {
                continue;
            }
            let mut matching_ids = true;
            for (before, after) in sessions.iter().zip(&current) {
                check_request(deadline, cancelled)?;
                if before.id != after.id {
                    matching_ids = false;
                    break;
                }
            }
            if !matching_ids {
                continue;
            }
            let context = match socket.as_deref() {
                Some(socket) => self.invoking_context(socket, deadline, cancelled)?,
                None => InvokingContext::OutsideTmux,
            };
            return finish_snapshot(sessions, context, deadline, cancelled);
        }
    }

    fn invoking_context(
        &self,
        socket: &OsStr,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<InvokingContext, AcquireError> {
        let Some(tty) = input_tty()? else {
            return Ok(InvokingContext::ActiveUnavailable);
        };
        if tty
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'\t' | b'\n'))
        {
            return Ok(InvokingContext::ActiveUnavailable);
        }
        let clients = self.request(
            Some(socket),
            &["list-clients", "-F", "#{client_tty}\t#{session_id}"],
            deadline,
            cancelled,
        )?;
        if !clients.status.success() {
            return Ok(InvokingContext::ActiveUnavailable);
        }
        parse_client_context(&clients.stdout, &tty, deadline, cancelled)
    }

    fn list(
        &self,
        socket: Option<&OsStr>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<Session>, AcquireError> {
        let output = self.request(
            socket,
            &[
                "list-sessions",
                "-F",
                "#{session_id}\t#{session_windows}\t#{session_created}\t#{window_id}\t#{window_width}\t#{window_height}",
            ],
            deadline,
            cancelled,
        )?;
        if !output.status.success() {
            if socket.is_none() && absent_server(&output.stderr) {
                return Ok(Vec::new());
            }
            return Err(command_error("session list", &output.stderr));
        }
        parse_sessions(&output.stdout, socket.is_some(), deadline, cancelled)
    }
}

fn finish_snapshot(
    sessions: Vec<Session>,
    context: InvokingContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SessionSnapshot, AcquireError> {
    SessionSnapshot::new_checked(sessions, SystemTime::now(), context, || {
        check_request(deadline, cancelled).map_err(AcquireError::from)
    })
}

fn parse_sessions(
    output: &[u8],
    inside_tmux: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<Session>, AcquireError> {
    check_request(deadline, cancelled)?;
    let text = std::str::from_utf8(output)
        .map_err(|_| AcquireError::Invalid("invalid tmux metadata encoding".to_owned()))?;
    check_request(deadline, cancelled)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(AcquireError::Invalid(
            "malformed tmux metadata framing".to_owned(),
        ));
    }
    let mut sessions = Vec::new();
    for line in text.split_terminator('\n') {
        check_request(deadline, cancelled)?;
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(AcquireError::Invalid(
                "malformed tmux metadata framing".to_owned(),
            ));
        }
        let id = SessionId::parse(fields[0]).map_err(AcquireError::Invalid)?;
        let created_at = (!fields[2].is_empty() && fields[2].bytes().all(|b| b.is_ascii_digit()))
            .then(|| fields[2].parse::<u64>().ok())
            .flatten()
            .and_then(|seconds| SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
            .map(Field::Known)
            .unwrap_or_else(|| unavailable("uptime", "invalid session creation time"));
        let current_window = match WindowId::parse(fields[3]) {
            Ok(id) => Field::Known(CurrentWindow {
                id,
                width: positive_field(fields[4], "width"),
                height: positive_field(fields[5], "height"),
            }),
            Err(_) => unavailable("size", "invalid current-window identity"),
        };
        sessions
            .try_reserve(1)
            .map_err(|_| ProcessError::ResourceExhaustion)?;
        sessions.push(Session {
            id,
            name: unavailable("name", "session name not retrieved"),
            window_count: positive_field(fields[1], "windows"),
            created_at,
            current_window,
            active: if inside_tmux {
                unavailable(
                    "active",
                    "cannot reliably identify the invoking tmux client",
                )
            } else {
                Field::Known(false)
            },
        });
    }
    sessions.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    check_request(deadline, cancelled)?;
    if sessions.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(AcquireError::Invalid(
            "duplicate tmux session identity".to_owned(),
        ));
    }
    Ok(sessions)
}

fn parse_client_context(
    output: &[u8],
    tty: &OsStr,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<InvokingContext, AcquireError> {
    check_request(deadline, cancelled)?;
    let Some(lines) = output.strip_suffix(b"\n") else {
        return Ok(InvokingContext::ActiveUnavailable);
    };
    let mut matched = None;
    for line in lines.split(|byte| *byte == b'\n') {
        check_request(deadline, cancelled)?;
        let Some(separator) = line.iter().position(|byte| *byte == b'\t') else {
            return Ok(InvokingContext::ActiveUnavailable);
        };
        let (client_tty, id) = line.split_at(separator);
        if client_tty != tty.as_bytes() {
            continue;
        }
        if matched.is_some() {
            return Ok(InvokingContext::ActiveUnavailable);
        }
        let Ok(id) = std::str::from_utf8(&id[1..]) else {
            return Ok(InvokingContext::ActiveUnavailable);
        };
        let Ok(id) = SessionId::parse(id) else {
            return Ok(InvokingContext::ActiveUnavailable);
        };
        matched = Some(id);
    }
    check_request(deadline, cancelled)?;
    Ok(match matched {
        Some(session_id) => InvokingContext::InsideTmux { session_id },
        None => InvokingContext::ActiveUnavailable,
    })
}

fn input_tty() -> Result<Option<OsString>, ProcessError> {
    let mut buffer = vec![0_u8; 128];
    loop {
        let result = unsafe {
            libc::ttyname_r(libc::STDIN_FILENO, buffer.as_mut_ptr().cast(), buffer.len())
        };
        match result {
            0 => {
                let Some(end) = buffer.iter().position(|byte| *byte == 0) else {
                    return Ok(None);
                };
                buffer.truncate(end);
                return Ok(Some(OsString::from_vec(buffer)));
            }
            libc::ERANGE => {
                let additional = buffer.len();
                let length = buffer
                    .len()
                    .checked_add(additional)
                    .ok_or(ProcessError::ResourceExhaustion)?;
                buffer
                    .try_reserve_exact(additional)
                    .map_err(|_| ProcessError::ResourceExhaustion)?;
                buffer.resize(length, 0);
            }
            _ => return Ok(None),
        }
    }
}

fn unavailable<T>(field: &str, message: &str) -> Field<T> {
    Field::Unavailable(FieldIssue {
        session_id: None,
        field: field.to_owned(),
        category: if field == "active" {
            IssueCategory::Context
        } else {
            IssueCategory::MalformedOutput
        },
        message: message.to_owned(),
        recovery: Some("Check tmux and retry.".to_owned()),
    })
}

fn parse_context(value: &OsStr) -> Result<OsString, AcquireError> {
    let mut parts = value.as_bytes().rsplitn(3, |byte| *byte == b',');
    let index = parts.next().unwrap_or_default();
    let pid = parts.next().unwrap_or_default();
    let socket = parts.next().unwrap_or_default();
    let digits = |part: &[u8]| !part.is_empty() && part.iter().all(u8::is_ascii_digit);
    if !digits(index)
        || !digits(pid)
        || socket.contains(&0)
        || !Path::new(OsStr::from_bytes(socket)).is_absolute()
    {
        return Err(AcquireError::Invalid(
            "invalid inherited tmux server context; check TMUX and retry".to_owned(),
        ));
    }
    Ok(OsString::from_vec(socket.to_vec()))
}

fn absent_server(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr);
    text.starts_with("no server running on ")
        || (text.starts_with("error connecting to ")
            && text.contains("(No such file or directory)"))
}

fn command_error(what: &str, stderr: &[u8]) -> AcquireError {
    let diagnostic = escape_text(String::from_utf8_lossy(stderr).trim());
    AcquireError::Invalid(format!(
        "tmux {what} failed: {diagnostic}; check the server and retry"
    ))
}

fn successful_line<'a>(output: &'a Output, what: &str) -> Result<&'a str, AcquireError> {
    if !output.status.success() {
        return Err(command_error(what, &output.stderr));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| AcquireError::Invalid(format!("unverifiable tmux {what}")))?;
    let text = text
        .strip_suffix('\n')
        .filter(|text| !text.contains('\n'))
        .ok_or_else(|| AcquireError::Invalid(format!("unverifiable tmux {what}")))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_and_snapshot_construction_reject_expired_or_cancelled_requests() {
        let metadata = b"$1\t1\t0\t@1\t80\t24\n";
        for (deadline, cancelled, expected_timeout) in [
            (Instant::now() - Duration::from_secs(1), false, true),
            (Instant::now() + Duration::from_secs(60), true, false),
        ] {
            let cancelled = AtomicBool::new(cancelled);
            let parse = parse_sessions(metadata, false, deadline, &cancelled);
            let snapshot = finish_snapshot(
                Vec::new(),
                InvokingContext::OutsideTmux,
                deadline,
                &cancelled,
            );
            let clients = parse_client_context(
                b"/dev/pts/2\t$1\n",
                OsStr::new("/dev/pts/2"),
                deadline,
                &cancelled,
            );
            for result in [parse.map(|_| ()), snapshot.map(|_| ()), clients.map(|_| ())] {
                if expected_timeout {
                    assert!(matches!(
                        result,
                        Err(AcquireError::Process(ProcessError::Timeout))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(AcquireError::Process(ProcessError::Cancelled))
                    ));
                }
            }
        }
    }

    #[test]
    fn snapshot_construction_checks_for_cancellation_during_processing() {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(60);
        let sessions = parse_sessions(
            b"$1\t1\t0\t@1\t80\t24\n$2\t1\t0\t@2\t80\t24\n$3\t1\t0\t@3\t80\t24\n",
            false,
            deadline,
            &cancelled,
        )
        .unwrap();
        let mut checks = 0;
        let result = SessionSnapshot::new_checked(
            sessions,
            SystemTime::now(),
            InvokingContext::OutsideTmux,
            || {
                checks += 1;
                if checks == 3 {
                    cancelled.store(true, Ordering::SeqCst);
                }
                check_request(deadline, &cancelled).map_err(AcquireError::from)
            },
        );
        assert!(matches!(
            result,
            Err(AcquireError::Process(ProcessError::Cancelled))
        ));
        assert_eq!(checks, 3);
    }

    #[test]
    fn client_identity_requires_one_exact_tty_match_with_valid_framing() {
        let tty = OsStr::new("/dev/pts/2");
        let deadline = Instant::now() + Duration::from_secs(60);
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            parse_client_context(
                b"/dev/pts/20\t$9\n/dev/pts/2\t$3\n",
                tty,
                deadline,
                &cancelled
            )
            .unwrap(),
            InvokingContext::InsideTmux {
                session_id: SessionId::parse("$3").unwrap(),
            }
        );
        for output in [
            b"".as_slice(),
            b"/dev/pts/20\t$9\n",
            b"/dev/pts/2\t$3",
            b"/dev/pts/2\t$3\n/dev/pts/2\t$4\n",
            b"/dev/pts/2\tinvalid\n",
            b"bad framing\n/dev/pts/2\t$3\n",
        ] {
            assert_eq!(
                parse_client_context(output, tty, deadline, &cancelled).unwrap(),
                InvokingContext::ActiveUnavailable,
                "{output:?}"
            );
        }
    }
}
