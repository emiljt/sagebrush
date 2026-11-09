mod support;

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use portable_pty::CommandBuilder;
use support::{IsolatedTmux, ProcessFixture, pty};

struct ChildGuard(Box<dyn portable_pty::Child + Send + Sync>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn termios(fd: i32) -> libc::termios {
    let mut value = std::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { libc::tcgetattr(fd, value.as_mut_ptr()) }, 0);
    unsafe { value.assume_init() }
}

struct TestScreen {
    cells: Vec<Vec<char>>,
    row: usize,
    col: usize,
    escape: u8,
    csi: String,
}

impl TestScreen {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            cells: vec![vec![' '; cols]; rows],
            row: 0,
            col: 0,
            escape: 0,
            csi: String::new(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match (self.escape, byte) {
                (0, 0x1b) => self.escape = 1,
                (1, b'[') => {
                    self.escape = 2;
                    self.csi.clear();
                }
                (1, _) => self.escape = 0,
                (2, 0x40..=0x7e) => {
                    self.control(char::from(byte));
                    self.escape = 0;
                }
                (2, _) => self.csi.push(char::from(byte)),
                (0, b'\r') => self.col = 0,
                (0, b'\n') => self.row = (self.row + 1).min(self.cells.len() - 1),
                (0, 0x20..=0x7e) => {
                    if let Some(cell) = self
                        .cells
                        .get_mut(self.row)
                        .and_then(|r| r.get_mut(self.col))
                    {
                        *cell = char::from(byte);
                    }
                    self.col += 1;
                }
                _ => {}
            }
        }
    }

    fn control(&mut self, final_byte: char) {
        if self.csi.starts_with('?') {
            return;
        }
        let values: Vec<usize> = self
            .csi
            .split(';')
            .map(|value| value.parse().unwrap_or(0))
            .collect();
        let first = values[0];
        match final_byte {
            'H' | 'f' => {
                self.row = first.max(1) - 1;
                self.col = values.get(1).copied().unwrap_or(0).max(1) - 1;
            }
            'G' => self.col = first.max(1) - 1,
            'd' => self.row = first.max(1) - 1,
            'A' => self.row = self.row.saturating_sub(first.max(1)),
            'B' => self.row = (self.row + first.max(1)).min(self.cells.len() - 1),
            'C' => self.col += first.max(1),
            'D' => self.col = self.col.saturating_sub(first.max(1)),
            'J' if first == 2 => self.cells.iter_mut().for_each(|row| row.fill(' ')),
            'K' => {
                if let Some(row) = self.cells.get_mut(self.row) {
                    for cell in row.iter_mut().skip(self.col) {
                        *cell = ' ';
                    }
                }
            }
            _ => {}
        }
    }

    fn text(&self) -> String {
        self.cells
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn wait_for_text(receiver: &Receiver<Vec<u8>>, screen: &mut TestScreen, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if screen.text().contains(needle) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining) {
            Ok(bytes) => screen.feed(&bytes),
            Err(error) => panic!("missing {needle:?}: {error}; output: {}", screen.text()),
        }
    }
}

#[test]
fn help_guides_navigation_detail_inspection_and_exit() {
    let server = IsolatedTmux::new().unwrap();
    server.start("alpha").unwrap();
    server.start("beta").unwrap();
    let pair = pty(80, 14).unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_sagebrush"));
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env_remove("TMUX_PANE");
    command.env("TERM", "xterm");
    let mut child = ChildGuard(pair.slave.spawn_command(command).unwrap());
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let mut screen = TestScreen::new(14, 80);
    wait_for_text(&receiver, &mut screen, "Name: alpha");
    writer.write_all(b"?").unwrap();
    writer.flush().unwrap();
    wait_for_text(&receiver, &mut screen, "Move list focus or scroll help.");
    writer.write_all(b"j").unwrap();
    writer.flush().unwrap();
    wait_for_text(
        &receiver,
        &mut screen,
        "The crawl moves one cell per second",
    );
    writer.write_all(b"k").unwrap();
    writer.flush().unwrap();
    wait_for_text(&receiver, &mut screen, "read-only tmux session browser");
    writer.write_all(b"\x1b").unwrap();
    writer.flush().unwrap();
    wait_for_text(&receiver, &mut screen, "Name: alpha");
    writer.write_all(b"j").unwrap();
    writer.flush().unwrap();
    wait_for_text(&receiver, &mut screen, "Name: beta");
    wait_for_text(&receiver, &mut screen, "Windows: 1");
    wait_for_text(&receiver, &mut screen, "Active: no");
    writer.write_all(b"k").unwrap();
    writer.flush().unwrap();
    wait_for_text(&receiver, &mut screen, "Name: alpha");
    writer.write_all(b"q").unwrap();
    writer.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "TUI did not exit");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.exit_code(), 0);
    drop(writer);
    drop(pair.master);
    reader_thread.join().unwrap();
}

fn run_exit_test(
    key: Option<&[u8]>,
    signal: Option<i32>,
    expected: u32,
    suspend: bool,
    blocking: bool,
    fail_query: bool,
) {
    let server = IsolatedTmux::new().unwrap();
    server.start("fixture").unwrap();
    let pair = pty(120, 30).unwrap();
    let fd = pair.master.as_raw_fd().unwrap();
    let before = termios(fd);
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_sagebrush"));
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env_remove("TMUX_PANE");
    command.env("TERM", "xterm");
    if fail_query {
        command.env("PATH", "");
    }
    let blocking_fixture = if blocking {
        let fixture =
            ProcessFixture::new("#!/bin/sh\necho $$ > \"$PID_FILE\"\nexec /bin/sleep 30\n")
                .unwrap();
        let directory = tempfile::tempdir().unwrap();
        symlink(fixture.path(), directory.path().join("tmux")).unwrap();
        let pid_file = directory.path().join("pid");
        command.env("PATH", directory.path());
        command.env("PID_FILE", &pid_file);
        Some((fixture, directory, pid_file))
    } else {
        None
    };
    let mut child = ChildGuard(pair.slave.spawn_command(command).unwrap());
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut output = Vec::new();
    let expected_text = if fail_query { "Failed" } else { "Sagebrush" };
    while !String::from_utf8_lossy(&output).contains(expected_text) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let bytes = receiver
            .recv_timeout(remaining)
            .expect("TUI did not render");
        output.extend(bytes);
    }
    let worker_pid = if let Some((_, _, pid_file)) = &blocking_fixture {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok(pid) = std::fs::read_to_string(pid_file)
                && let Ok(pid) = pid.trim().parse::<i32>()
            {
                break Some(pid);
            }
            assert!(Instant::now() < deadline, "blocking query did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
    } else {
        None
    };
    if suspend {
        writer.write_all(b"\x1a").unwrap();
        writer.flush().unwrap();
        let pid = child.0.process_id().unwrap() as i32;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let mut status = 0;
            let result =
                unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG | libc::WUNTRACED) };
            if result == pid && libc::WIFSTOPPED(status) {
                break;
            }
            assert_ne!(result, -1, "waitpid failed");
            assert!(Instant::now() < deadline, "TUI did not suspend");
            std::thread::sleep(Duration::from_millis(10));
        }
        let stopped = termios(fd);
        assert_eq!(stopped.c_lflag, before.c_lflag);
        assert_eq!(stopped.c_iflag, before.c_iflag);
        assert_eq!(stopped.c_oflag, before.c_oflag);
        assert_eq!(stopped.c_cc, before.c_cc);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !output.windows(6).any(|bytes| bytes == b"\x1b[?25h") {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let bytes = receiver
                .recv_timeout(remaining)
                .expect("cursor was not restored");
            output.extend(bytes);
        }
        assert_eq!(unsafe { libc::kill(pid, libc::SIGCONT) }, 0);
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if termios(fd).c_lflag & libc::ICANON == 0 {
                break;
            }
            assert!(Instant::now() < deadline, "TUI did not resume raw mode");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    if let Some(key) = key {
        writer.write_all(key).unwrap();
        writer.flush().unwrap();
        if signal.is_some() {
            std::thread::sleep(Duration::from_millis(150));
        }
    }
    if let Some(signal) = signal {
        let pid = child.0.process_id().unwrap();
        assert_eq!(unsafe { libc::kill(pid as i32, signal) }, 0);
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "TUI did not exit");
        std::thread::sleep(Duration::from_millis(10));
    };
    for bytes in receiver.try_iter() {
        output.extend(bytes);
    }
    assert_eq!(
        status.exit_code(),
        expected,
        "terminal output: {}",
        String::from_utf8_lossy(&output)
    );
    let after = termios(fd);
    assert_eq!(after.c_lflag, before.c_lflag);
    assert_eq!(after.c_iflag, before.c_iflag);
    assert_eq!(after.c_oflag, before.c_oflag);
    assert_eq!(after.c_cc, before.c_cc);
    if let Some(pid) = worker_pid {
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
    drop(writer);
    drop(pair.master);
    reader_thread.join().unwrap();
}

#[test]
fn q_restores_terminal_and_exits_successfully() {
    run_exit_test(Some(b"q"), None, 0, false, false, false);
}

#[test]
fn control_c_restores_terminal_and_returns_130() {
    run_exit_test(Some(b"\x03"), None, 130, false, false, false);
}

#[test]
fn sigterm_restores_terminal_and_returns_143() {
    run_exit_test(None, Some(libc::SIGTERM), 143, false, false, false);
}

#[test]
fn incomplete_escape_sequence_does_not_block_termination() {
    run_exit_test(
        Some(b"\x1b["),
        Some(libc::SIGTERM),
        143,
        false,
        false,
        false,
    );
}

#[test]
fn sighup_restores_terminal_and_returns_129() {
    run_exit_test(None, Some(libc::SIGHUP), 129, false, false, false);
}

#[test]
fn control_z_restores_terminal_before_suspend_and_reenters_on_resume() {
    run_exit_test(Some(b"q"), None, 0, true, false, false);
}

#[test]
fn quitting_during_blocked_query_cancels_and_reaps_the_worker() {
    run_exit_test(Some(b"qq"), None, 0, false, true, false);
}

#[test]
fn q_after_recoverable_query_failure_exits_successfully() {
    run_exit_test(Some(b"q"), None, 0, false, false, true);
}

#[test]
fn terminal_disconnect_reports_cleanup_failure() {
    let server = IsolatedTmux::new().unwrap();
    server.start("fixture").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let errors = directory.path().join("errors");
    let pair = pty(120, 30).unwrap();
    let fd = pair.master.as_raw_fd().unwrap();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert_ne!(flags, -1);
    assert_ne!(
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
        -1
    );
    let mut command = CommandBuilder::new("/bin/sh");
    command.args(["-c", "exec \"$APP\" 2>\"$ERRORS\""]);
    command.env("APP", env!("CARGO_BIN_EXE_sagebrush"));
    command.env("ERRORS", &errors);
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env("TERM", "xterm");
    let mut child = ChildGuard(pair.slave.spawn_command(command).unwrap());
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !String::from_utf8_lossy(&output).contains("Sagebrush") {
        let mut bytes = [0; 4096];
        match reader.read(&mut bytes) {
            Ok(0) => panic!("terminal closed before rendering"),
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("terminal read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "TUI did not render");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(reader);
    drop(pair.master);
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "TUI did not exit after terminal disconnect"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.exit_code(), 1);
    let diagnostic = std::fs::read_to_string(errors).unwrap();
    assert!(diagnostic.contains("Terminal error"), "{diagnostic}");
}

fn split_pty() -> (std::fs::File, std::fs::File) {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 30,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!(size),
            )
        },
        0
    );
    let master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    assert_eq!(
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
        0
    );
    (master, slave)
}

#[test]
fn failed_alternate_screen_setup_restores_raw_mode() {
    let (input_master, input_slave) = split_pty();
    let (_output_master, output_slave) = split_pty();
    let before = termios(input_master.as_raw_fd());
    let mut name = [0; 1024];
    assert_eq!(
        unsafe { libc::ttyname_r(output_slave.as_raw_fd(), name.as_mut_ptr(), name.len()) },
        0
    );
    let path = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) };
    let read_only_output = std::fs::File::open(std::path::Path::new(std::ffi::OsStr::from_bytes(
        path.to_bytes(),
    )))
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
        .env("TERM", "xterm")
        .stdin(Stdio::from(input_slave))
        .stdout(Stdio::from(read_only_output))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("TUI did not exit after partial initialization");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("Terminal error"));
    let after = termios(input_master.as_raw_fd());
    assert_eq!(after.c_lflag, before.c_lflag);
    assert_eq!(after.c_iflag, before.c_iflag);
    assert_eq!(after.c_oflag, before.c_oflag);
    assert_eq!(after.c_cc, before.c_cc);
}

#[test]
fn draw_failure_restores_input_terminal() {
    let server = IsolatedTmux::new().unwrap();
    server.start("fixture").unwrap();
    let (input_master, input_slave) = split_pty();
    let (mut output_master, output_slave) = split_pty();
    let input_before = termios(input_master.as_raw_fd());
    let mut child = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
        .env("TMUX_TMPDIR", server.directory_path())
        .env_remove("TMUX")
        .env("TERM", "xterm")
        .stdin(Stdio::from(input_slave))
        .stdout(Stdio::from(output_slave))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let flags = unsafe { libc::fcntl(output_master.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(flags, -1);
    assert_ne!(
        unsafe {
            libc::fcntl(
                output_master.as_raw_fd(),
                libc::F_SETFL,
                flags | libc::O_NONBLOCK,
            )
        },
        -1
    );
    let mut rendered = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !String::from_utf8_lossy(&rendered).contains("Sagebrush") {
        let mut buffer = [0; 4096];
        match output_master.read(&mut buffer) {
            Ok(0) => panic!("output closed before rendering"),
            Ok(count) => rendered.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("cannot read output: {error}"),
        }
        assert!(Instant::now() < deadline, "TUI did not render");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(output_master);
    let deadline = Instant::now() + Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("TUI did not exit after output disconnect");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("Terminal error"));
    let after = termios(input_master.as_raw_fd());
    assert_eq!(after.c_lflag, input_before.c_lflag);
    assert_eq!(after.c_iflag, input_before.c_iflag);
    assert_eq!(after.c_oflag, input_before.c_oflag);
    assert_eq!(after.c_cc, input_before.c_cc);
}
