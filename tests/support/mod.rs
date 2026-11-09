#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use portable_pty::{NativePtySystem, PtyPair, PtySize, PtySystem};
use tempfile::TempDir;

#[derive(Clone)]
pub struct TestClock(Arc<Mutex<SystemTime>>);

impl TestClock {
    pub fn new(now: SystemTime) -> Self {
        Self(Arc::new(Mutex::new(now)))
    }

    pub fn now(&self) -> SystemTime {
        *self.0.lock().expect("test clock poisoned")
    }

    pub fn advance(&self, duration: Duration) {
        let mut now = self.0.lock().expect("test clock poisoned");
        *now = now.checked_add(duration).expect("test clock overflow");
    }
}

pub struct IsolatedTmux {
    directory: TempDir,
    running: Cell<bool>,
    sockets: RefCell<Vec<PathBuf>>,
}

impl IsolatedTmux {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            directory: tempfile::tempdir()?,
            running: Cell::new(false),
            sockets: RefCell::new(Vec::new()),
        })
    }

    pub fn directory_path(&self) -> &Path {
        self.directory.path()
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        command.env("TMUX_TMPDIR", self.directory.path());
        command.env_remove("TMUX").env_remove("TMUX_PANE");
        command
    }

    pub fn application(&self, executable: &Path) -> Command {
        let mut command = Command::new(executable);
        command.env_remove("TMUX").env_remove("TMUX_PANE");
        command.env("TMUX_TMPDIR", self.directory.path());
        command
    }

    pub fn start(&self, name: &str) -> io::Result<()> {
        let output = self
            .command()
            .args(["new-session", "-d", "-s", name])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "isolated tmux startup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        self.running.set(true);
        Ok(())
    }

    pub fn stop(&self) -> io::Result<()> {
        let output = self.command().arg("kill-server").output()?;
        if output.status.success() {
            self.running.set(false);
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "isolated tmux cleanup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    pub fn start_on_socket(&self, name: &str) -> io::Result<PathBuf> {
        let socket = self.directory.path().join("custom-socket");
        let output = self
            .command()
            .arg("-S")
            .arg(&socket)
            .args(["new-session", "-d", "-s", name])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "isolated tmux startup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        self.sockets.borrow_mut().push(socket.clone());
        Ok(socket)
    }
}

impl Drop for IsolatedTmux {
    fn drop(&mut self) {
        for socket in self.sockets.borrow().iter() {
            match self
                .command()
                .arg("-S")
                .arg(socket)
                .arg("kill-server")
                .output()
            {
                Ok(output) if output.status.success() => {}
                Ok(output) => eprintln!(
                    "isolated tmux cleanup failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                Err(error) => eprintln!("isolated tmux cleanup failed: {error}"),
            }
        }
        if self.running.get()
            && let Err(error) = self.stop()
        {
            eprintln!("{error}");
        }
    }
}

pub struct ProcessFixture {
    directory: TempDir,
}

impl ProcessFixture {
    pub fn new(script: &str) -> io::Result<Self> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("fixture");
        std::fs::write(&path, script)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { directory })
    }

    pub fn command(&self) -> Command {
        Command::new(self.directory.path().join("fixture"))
    }

    pub fn path(&self) -> PathBuf {
        self.directory.path().join("fixture")
    }
}

pub fn pty(columns: u16, rows: u16) -> io::Result<PtyPair> {
    NativePtySystem::default()
        .openpty(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| io::Error::other(error.to_string()))
}
