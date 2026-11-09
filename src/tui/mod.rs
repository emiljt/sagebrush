pub mod state;
pub mod view;

use std::io::{self, Write, stdout};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    model::SessionSnapshot,
    tmux::{AcquireError, ProcessError, TmuxClient},
};
use state::AppState;

trait TerminalControl {
    fn enable_raw(&mut self) -> io::Result<()>;
    fn disable_raw(&mut self) -> io::Result<()>;
    fn enter_alternate(&mut self) -> io::Result<()>;
    fn leave_alternate(&mut self) -> io::Result<()>;
}

struct CrosstermControl;

impl TerminalControl for CrosstermControl {
    fn enable_raw(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut attributes = std::mem::MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, attributes.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = unsafe { attributes.assume_init() };
        // The event parser may request another read for an incomplete escape sequence.
        attributes.c_cc[libc::VMIN] = 0;
        attributes.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &attributes) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        let flags = unsafe { libc::fcntl(libc::STDOUT_FILENO, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if flags & libc::O_ACCMODE == libc::O_RDONLY {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "terminal output is not writable",
            ));
        }
        execute!(stdout(), EnterAlternateScreen)
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        execute!(stdout(), LeaveAlternateScreen)
    }
}

struct TerminalOwner<C: TerminalControl> {
    control: C,
    raw: bool,
    alternate: bool,
}

impl<C: TerminalControl> TerminalOwner<C> {
    fn new(control: C) -> Self {
        Self {
            control,
            raw: false,
            alternate: false,
        }
    }

    fn enter(&mut self) -> io::Result<()> {
        self.raw = true;
        self.control.enable_raw()?;
        self.alternate = true;
        self.control.enter_alternate()?;
        Ok(())
    }

    fn restore(&mut self) -> io::Result<()> {
        let alternate = if self.alternate {
            let result = self.control.leave_alternate();
            if result.is_ok() {
                self.alternate = false;
            }
            result
        } else {
            Ok(())
        };
        let raw = if self.raw {
            let result = self.control.disable_raw();
            if result.is_ok() {
                self.raw = false;
            }
            result
        } else {
            Ok(())
        };
        match (alternate, raw) {
            (Err(first), Err(second)) => Err(io::Error::other(format!(
                "cannot restore alternate screen: {first}; cannot restore terminal mode: {second}"
            ))),
            (Err(error), _) | (_, Err(error)) => Err(error),
            _ => Ok(()),
        }
    }
}

impl<C: TerminalControl> Drop for TerminalOwner<C> {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            let _ = writeln!(io::stderr(), "Cannot restore terminal: {error}");
        }
    }
}

struct WorkerGuard {
    thread: Option<JoinHandle<()>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.cancelled.store(true, Ordering::SeqCst);
            if thread.join().is_err() {
                let _ = writeln!(io::stderr(), "tmux worker terminated unexpectedly");
            }
        }
    }
}

pub fn run(
    no_color: bool,
    cancelled: Arc<AtomicBool>,
    signal: Arc<AtomicI32>,
    suspend: Arc<AtomicBool>,
) -> io::Result<i32> {
    let mut owner = TerminalOwner::new(CrosstermControl);
    if let Err(error) = owner.enter() {
        return match owner.restore() {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; cleanup failed: {cleanup}"
            ))),
        };
    }
    let result = match Terminal::new(CrosstermBackend::new(stdout())) {
        Ok(mut terminal) => {
            let result = event_loop(
                &mut terminal,
                &mut owner,
                no_color,
                cancelled,
                signal,
                suspend,
            );
            match (result, terminal.show_cursor()) {
                (Err(error), Err(cursor)) => Err(io::Error::other(format!(
                    "{error}; cannot restore cursor: {cursor}"
                ))),
                (_, Err(error)) | (Err(error), _) => Err(error),
                (Ok(status), Ok(())) => Ok(status),
            }
        }
        Err(error) => Err(error),
    };
    match (result, owner.restore()) {
        (Err(error), Err(cleanup)) => Err(io::Error::other(format!(
            "{error}; terminal cleanup failed: {cleanup}"
        ))),
        (_, Err(error)) | (Err(error), _) => Err(error),
        (Ok(status), Ok(())) => Ok(status),
    }
}

type WorkerResult = Result<SessionSnapshot, AcquireError>;

fn finish_worker(
    worker: JoinHandle<()>,
    receiver: &mpsc::Receiver<WorkerResult>,
) -> io::Result<()> {
    worker
        .join()
        .map_err(|_| io::Error::other("tmux worker terminated unexpectedly"))?;
    if let Ok(Err(AcquireError::Process(ProcessError::Cleanup(error)))) = receiver.try_recv() {
        return Err(io::Error::other(format!(
            "tmux subprocess cleanup failed: {error}"
        )));
    }
    Ok(())
}

fn check_terminal_streams() -> io::Result<()> {
    let mut fds = [
        libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: libc::STDOUT_FILENO,
            events: libc::POLLOUT,
            revents: 0,
        },
    ];
    if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    for (fd, stream) in fds.iter().zip(["input", "output"]) {
        if fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("terminal {stream} disconnected"),
            ));
        }
    }
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    owner: &mut TerminalOwner<CrosstermControl>,
    no_color: bool,
    cancelled: Arc<AtomicBool>,
    signal: Arc<AtomicI32>,
    suspend: Arc<AtomicBool>,
) -> io::Result<i32> {
    let mut state = AppState::new(Instant::now());
    let (sender, receiver) = mpsc::channel::<WorkerResult>();
    let mut worker = WorkerGuard {
        thread: None,
        cancelled: Arc::clone(&cancelled),
    };
    let color = view::color_enabled(
        no_color,
        std::env::var_os("NO_COLOR").as_deref(),
        std::env::var_os("TERM").as_deref(),
    );
    let mut failure = None;
    let mut status = 0;
    loop {
        match check_terminal_streams() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
        let number = signal.load(Ordering::SeqCst);
        if number != 0 {
            status = 128 + number;
            break;
        }
        if suspend.swap(false, Ordering::SeqCst)
            && let Err(error) = suspend_terminal(owner, terminal)
        {
            failure = Some(error);
            break;
        }
        let now = Instant::now();
        if let Ok(result) = receiver.try_recv() {
            if let Some(thread) = worker.thread.take()
                && thread.join().is_err()
            {
                state.finish_request(now, Err("tmux worker terminated unexpectedly".into()));
            } else {
                state.finish_request(now, result.map_err(|error| error.to_string()));
            }
        } else if worker.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            let thread = worker.thread.take().expect("finished worker must exist");
            let result = if thread.join().is_err() {
                Err(AcquireError::Invalid(
                    "tmux worker terminated unexpectedly".into(),
                ))
            } else {
                receiver.try_recv().unwrap_or_else(|_| {
                    Err(AcquireError::Invalid(
                        "tmux worker returned no result".into(),
                    ))
                })
            };
            state.finish_request(now, result.map_err(|error| error.to_string()));
        }
        if state.begin_due_request(now) {
            let sender = sender.clone();
            let cancelled = Arc::clone(&cancelled);
            let thread = thread::Builder::new()
                .name("tmux-request".into())
                .spawn(move || {
                    let result = TmuxClient::from_environment().acquire(&cancelled);
                    let _ = sender.send(result);
                });
            match thread {
                Ok(thread) => worker.thread = Some(thread),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        if let Err(error) = terminal.draw(|frame| view::render(frame, &mut state, now, color)) {
            failure = Some(error);
            break;
        }
        match event::poll(Duration::from_millis(50)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        status = 130;
                        break;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('z')
                    {
                        suspend.store(true, Ordering::SeqCst);
                    } else if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                        && state.handle_key(key.code, Instant::now())
                    {
                        break;
                    }
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            },
            Ok(false) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    cancelled.store(true, Ordering::SeqCst);
    if let Some(worker) = worker.thread.take() {
        while !worker.is_finished() {
            if failure.is_none()
                && let Err(error) = terminal.draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(
                            "Cleaning up... press q again to continue waiting.",
                        ),
                        area,
                    );
                })
            {
                failure = Some(error);
            }
            if failure.is_none() {
                match event::poll(Duration::from_millis(50)) {
                    Ok(true) => match event::read() {
                        Ok(Event::Key(key))
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && key.code == KeyCode::Char('c') =>
                        {
                            status = 130;
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        Err(error) => failure = Some(error),
                    },
                    Ok(false) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => failure = Some(error),
                }
                let number = signal.load(Ordering::SeqCst);
                if number != 0 {
                    status = 128 + number;
                }
            } else {
                thread::sleep(Duration::from_millis(50));
            }
            let number = signal.load(Ordering::SeqCst);
            if number != 0 {
                status = 128 + number;
            }
        }
        if let Err(error) = finish_worker(worker, &receiver) {
            failure = Some(match failure {
                Some(previous) => io::Error::other(format!("{previous}; {error}")),
                None => error,
            });
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(status),
    }
}

fn suspend_terminal(
    owner: &mut TerminalOwner<CrosstermControl>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<()> {
    terminal.show_cursor()?;
    owner.restore()?;
    if unsafe { libc::kill(libc::getpid(), libc::SIGSTOP) } != 0 {
        return Err(io::Error::last_os_error());
    }
    owner.enter()?;
    execute!(
        stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0)
    )?;
    terminal.swap_buffers();
    terminal.swap_buffers();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    #[test]
    fn shutdown_reports_subprocess_cleanup_failure_but_not_cancellation() {
        for (result, expected_failure) in [
            (
                Err(AcquireError::Process(ProcessError::Cleanup(
                    io::Error::other("reap failed"),
                ))),
                true,
            ),
            (Err(AcquireError::Process(ProcessError::Cancelled)), false),
        ] {
            let (sender, receiver) = mpsc::channel();
            let worker = thread::spawn(move || sender.send(result).unwrap());
            let outcome = finish_worker(worker, &receiver);
            if expected_failure {
                assert!(outcome.unwrap_err().to_string().contains("reap failed"));
            } else {
                outcome.unwrap();
            }
        }
    }

    struct FakeControl {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail_once: Vec<&'static str>,
    }

    impl FakeControl {
        fn call(&mut self, step: &'static str) -> io::Result<()> {
            self.calls.borrow_mut().push(step);
            if let Some(index) = self.fail_once.iter().position(|failed| *failed == step) {
                self.fail_once.remove(index);
                Err(io::Error::other(format!("{step} failed")))
            } else {
                Ok(())
            }
        }
    }

    impl TerminalControl for FakeControl {
        fn enable_raw(&mut self) -> io::Result<()> {
            self.call("enable")
        }

        fn disable_raw(&mut self) -> io::Result<()> {
            self.call("disable")
        }

        fn enter_alternate(&mut self) -> io::Result<()> {
            self.call("enter")
        }

        fn leave_alternate(&mut self) -> io::Result<()> {
            self.call("leave")
        }
    }

    fn owner(
        fail_once: Vec<&'static str>,
    ) -> (TerminalOwner<FakeControl>, Rc<RefCell<Vec<&'static str>>>) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        (
            TerminalOwner::new(FakeControl {
                calls: Rc::clone(&calls),
                fail_once,
            }),
            calls,
        )
    }

    #[test]
    fn partial_initialization_attempts_restoration() {
        for (failed, expected) in [
            ("enable", vec!["enable", "disable"]),
            ("enter", vec!["enable", "enter", "leave", "disable"]),
        ] {
            let (mut owner, calls) = owner(vec![failed]);
            assert!(owner.enter().is_err());
            owner.restore().unwrap();
            assert_eq!(*calls.borrow(), expected);
        }
    }

    #[test]
    fn cleanup_reports_both_failures_and_retries_on_drop() {
        let (mut owner, calls) = owner(vec!["leave", "disable"]);
        owner.enter().unwrap();
        let error = owner.restore().unwrap_err().to_string();
        assert!(
            error.contains("leave failed") && error.contains("disable failed"),
            "{error}"
        );
        drop(owner);
        assert_eq!(
            *calls.borrow(),
            ["enable", "enter", "leave", "disable", "leave", "disable"]
        );
    }

    #[test]
    fn unwind_restores_terminal_ownership() {
        let (mut owner, calls) = owner(vec![]);
        owner.enter().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("simulated draw failure");
        }));
        assert!(result.is_err());
        assert_eq!(*calls.borrow(), ["enable", "enter", "leave", "disable"]);
    }
}
