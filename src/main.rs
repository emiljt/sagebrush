use std::io::{self, IsTerminal, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use clap::Parser;
use sagebrush::cli::{Cli, Command};
use sagebrush::{output, tmux::TmuxClient};

struct Signals {
    cancelled: Arc<AtomicBool>,
    signal: Arc<AtomicI32>,
    suspend: Arc<AtomicBool>,
}

impl Signals {
    fn install(interactive: bool) -> io::Result<Self> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::new(AtomicI32::new(0));
        for number in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            let cancelled = Arc::clone(&cancelled);
            let signal = Arc::clone(&signal);
            // The handler only performs lock-free atomic stores.
            unsafe {
                signal_hook::low_level::register(number, move || {
                    signal.store(number, Ordering::SeqCst);
                    cancelled.store(true, Ordering::SeqCst);
                })?;
            }
        }
        let suspend = Arc::new(AtomicBool::new(false));
        if interactive {
            signal_hook::flag::register(libc::SIGTSTP, Arc::clone(&suspend))?;
        }
        Ok(Self {
            cancelled,
            signal,
            suspend,
        })
    }

    fn status(&self) -> Option<i32> {
        let signal = self.signal.load(Ordering::SeqCst);
        (signal != 0).then_some(128 + signal)
    }
}

fn main() {
    let cli = Cli::parse();
    let signals = match Signals::install(!matches!(cli.command, Some(Command::List { .. }))) {
        Ok(signals) => signals,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "Cannot install signal handlers: {error}"
            );
            std::process::exit(1);
        }
    };
    let status = match cli.command {
        Some(Command::List { json, plain }) => list(&signals, json, plain),
        Some(Command::Tui) | None => {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                let _ = writeln!(
                    io::stderr().lock(),
                    "The TUI requires terminal stdin and stdout. Use `sagebrush list` for redirected output; see --help."
                );
                2
            } else {
                match sagebrush::tui::run(
                    cli.no_color,
                    Arc::clone(&signals.cancelled),
                    Arc::clone(&signals.signal),
                    Arc::clone(&signals.suspend),
                ) {
                    Ok(status) => status,
                    Err(error) => {
                        let _ = writeln!(io::stderr().lock(), "Terminal error: {error}");
                        1
                    }
                }
            }
        }
    };
    std::process::exit(status);
}

fn list(signals: &Signals, json: bool, plain: bool) -> i32 {
    let result = TmuxClient::from_environment().acquire(&signals.cancelled);
    if let Some(status) = signals.status() {
        let _ = writeln!(io::stderr().lock(), "tmux request cancelled");
        return status;
    }
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "Cannot list tmux sessions: {error}");
            return 1;
        }
    };
    let text = if json {
        match output::json(&snapshot) {
            Ok(text) => text,
            Err(error) => {
                let _ = writeln!(io::stderr().lock(), "Cannot encode session list: {error}");
                return 1;
            }
        }
    } else if plain {
        output::plain(&snapshot)
    } else {
        output::table(&snapshot, io::stdout().is_terminal())
    };
    match write_output(text.as_bytes(), signals) {
        Ok(Some(status)) => {
            let _ = writeln!(io::stderr().lock(), "tmux request cancelled");
            return status;
        }
        Ok(None) => {}
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return 141,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "Cannot write session list: {error}");
            return 1;
        }
    }
    if !snapshot.issues.is_empty() {
        let _ = io::stderr()
            .lock()
            .write_all(output::diagnostics(&snapshot).as_bytes());
        return 1;
    }
    0
}

struct OutputMode {
    original: i32,
    changed: bool,
}

impl OutputMode {
    fn nonblocking() -> io::Result<Self> {
        let original = unsafe { libc::fcntl(libc::STDOUT_FILENO, libc::F_GETFL) };
        if original == -1 {
            return Err(io::Error::last_os_error());
        }
        let changed = original & libc::O_NONBLOCK == 0;
        if changed
            && unsafe {
                libc::fcntl(
                    libc::STDOUT_FILENO,
                    libc::F_SETFL,
                    original | libc::O_NONBLOCK,
                )
            } == -1
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { original, changed })
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.changed {
            if unsafe { libc::fcntl(libc::STDOUT_FILENO, libc::F_SETFL, self.original) } == -1 {
                return Err(io::Error::last_os_error());
            }
            self.changed = false;
        }
        Ok(())
    }
}

impl Drop for OutputMode {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            let _ = writeln!(io::stderr(), "Cannot restore stdout mode: {error}");
        }
    }
}

fn write_output(bytes: &[u8], signals: &Signals) -> io::Result<Option<i32>> {
    if bytes.is_empty() {
        return Ok(signals.status());
    }
    let mut mode = OutputMode::nonblocking()?;
    let result = write_nonblocking(bytes, signals);
    match (result, mode.restore()) {
        (Err(error), Err(restore)) => Err(io::Error::other(format!(
            "{error}; cannot restore stdout mode: {restore}"
        ))),
        (_, Err(error)) | (Err(error), _) => Err(error),
        (Ok(status), Ok(())) => Ok(status),
    }
}

fn write_nonblocking(mut bytes: &[u8], signals: &Signals) -> io::Result<Option<i32>> {
    while !bytes.is_empty() {
        if let Some(status) = signals.status() {
            return Ok(Some(status));
        }
        let written = unsafe {
            libc::write(
                libc::STDOUT_FILENO,
                bytes.as_ptr().cast(),
                bytes.len().min(64 * 1024),
            )
        };
        if written > 0 {
            bytes = &bytes[written as usize..];
            continue;
        }
        if written == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "stdout write returned zero",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
        let mut poll = libc::pollfd {
            fd: libc::STDOUT_FILENO,
            events: libc::POLLOUT,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 50) } == -1 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    Ok(signals.status())
}
