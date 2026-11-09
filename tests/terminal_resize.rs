mod support;

use std::io::{Read, Write};
use std::os::unix::fs::symlink;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize};
use support::{ProcessFixture, pty};

struct ChildGuard(Box<dyn portable_pty::Child + Send + Sync>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn wait_for(receiver: &Receiver<Vec<u8>>, needle: &str) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut output = Vec::new();
    loop {
        if String::from_utf8_lossy(&output).contains(needle) {
            return output;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining) {
            Ok(bytes) => output.extend(bytes),
            Err(error) => panic!(
                "missing {needle:?}: {error}; output: {}",
                String::from_utf8_lossy(&output)
            ),
        }
    }
}

#[test]
fn resize_help_and_quit_remain_responsive_during_blocked_refresh() {
    let fixture = ProcessFixture::new(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$LOG\"\nshift\ncase \"$1\" in -V) if [ -e \"$BLOCK\" ]; then echo $$ > \"$PID_FILE\"; exec /bin/sleep 30; else echo 'tmux 3.4'; fi;; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n$2\\t1\\t0\\t@2\\t80\\t24\\n';; display-message) if [ \"$3\" != -t ]; then echo '3.4'; elif [ \"$4\" = '$1' ]; then echo first; else echo second; fi;; esac\n",
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    symlink(fixture.path(), directory.path().join("tmux")).unwrap();
    let block = directory.path().join("block");
    let pid_file = directory.path().join("pid");
    let log = directory.path().join("log");
    let pair = pty(120, 30).unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_sagebrush"));
    command.env("PATH", directory.path());
    command.env("BLOCK", &block);
    command.env("PID_FILE", &pid_file);
    command.env("LOG", &log);
    command.env("TERM", "xterm");
    command.env_remove("TMUX");
    let mut child = ChildGuard(pair.slave.spawn_command(command).unwrap());
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader_thread = std::thread::spawn(move || {
        let mut bytes = [0; 4096];
        while let Ok(count) = reader.read(&mut bytes) {
            if count == 0 || sender.send(bytes[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    wait_for(&receiver, "second");
    std::fs::write(&block, "").unwrap();
    writer.write_all(b"j?").unwrap();
    writer.flush().unwrap();
    wait_for(&receiver, "Help");
    let deadline = Instant::now() + Duration::from_secs(8);
    while !pid_file.exists() {
        assert!(Instant::now() < deadline, "refresh did not block");
        std::thread::sleep(Duration::from_millis(10));
    }
    let calls = std::fs::read_to_string(&log).unwrap().lines().count();
    pair.master
        .resize(PtySize {
            rows: 10,
            cols: 39,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    wait_for(&receiver, "Terminal");
    pair.master
        .resize(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    wait_for(&receiver, "Help");
    writer.write_all(b"\x1b").unwrap();
    writer.flush().unwrap();
    let restored = wait_for(&receiver, "second");
    let restored = String::from_utf8_lossy(&restored);
    let first = restored.find("first").expect("first row was not redrawn");
    let second = restored.find("second").unwrap();
    assert!(
        restored[first..second].contains('>'),
        "focus was not preserved: {restored}"
    );
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().lines().count(),
        calls
    );
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
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    drop(writer);
    drop(pair.master);
    reader_thread.join().unwrap();
}
