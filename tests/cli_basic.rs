mod support;

use std::os::unix::fs::symlink;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use support::ProcessFixture;

fn app() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sagebrush"));
    command.env_remove("TMUX").env_remove("TMUX_PANE");
    command
}

fn fake_tmux(script: &str) -> (ProcessFixture, tempfile::TempDir) {
    let fixture = ProcessFixture::new(script).unwrap();
    let directory = tempfile::tempdir().unwrap();
    symlink(fixture.path(), directory.path().join("tmux")).unwrap();
    (fixture, directory)
}

#[test]
fn help_and_version_do_not_need_tmux() {
    for args in [
        vec!["--help"],
        vec!["--version"],
        vec!["list", "--help"],
        vec!["tui", "--help"],
    ] {
        let output = app().args(&args).env("PATH", "").output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn redirected_tui_launches_are_rejected_without_querying_tmux() {
    for args in [vec![], vec!["tui"], vec!["--no-color", "tui"]] {
        let output = app()
            .args(&args)
            .env("PATH", "")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains("terminal") && message.contains("list"),
            "{message}"
        );
    }
}

#[test]
fn basic_listing_preserves_fields_and_escapes_control_characters() {
    let (_fixture, directory) = fake_tmux(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t2\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = -t ]; then printf ' a\\tb\\\\c\\n'; else echo '3.4'; fi;; esac\n",
    );
    let output = app()
        .arg("list")
        .env("PATH", directory.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let result = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "NAME",
        "WINDOWS",
        "UPTIME",
        "SIZE",
        "ACTIVE",
        " a\\tb\\\\c",
        "80x24",
        "no",
    ] {
        assert!(result.contains(expected), "{expected}: {result}");
    }
}

#[test]
fn empty_partial_and_fatal_results_have_distinct_streams_and_statuses() {
    let (_fixture, directory) = fake_tmux(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; *) echo 'no server running on /tmp/example' >&2; exit 1;; esac\n",
    );
    let output = app()
        .arg("list")
        .env("PATH", directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());

    let (_fixture, directory) = fake_tmux(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t0\\t24\\n';; display-message) if [ \"$3\" = -t ]; then echo test; else echo '3.4'; fi;; esac\n",
    );
    let output = app()
        .arg("list")
        .env("PATH", directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("test"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("width"));

    let output = app().arg("list").env("PATH", "").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("tmux"));
}

#[test]
fn closed_output_pipe_exits_without_panicking() {
    let (_fixture, directory) = fake_tmux(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = -t ]; then echo test; else echo '3.4'; fi;; esac\n",
    );
    let mut child = app()
        .arg("list")
        .env("PATH", directory.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(141), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panic"));
}

#[test]
fn signals_cancel_blocked_queries_without_partial_output() {
    for (signal, expected) in [
        (libc::SIGINT, 130),
        (libc::SIGHUP, 129),
        (libc::SIGTERM, 143),
    ] {
        let (_fixture, directory) =
            fake_tmux("#!/bin/sh\necho $$ > \"$PID_FILE\"\nexec /bin/sleep 30\n");
        let pid_file = directory.path().join("pid");
        let mut child = app()
            .arg("list")
            .env("PATH", directory.path())
            .env("PID_FILE", &pid_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        let worker_pid = loop {
            if let Ok(pid) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = pid.trim().parse::<i32>()
            {
                break pid;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("blocked tmux process did not start");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(expected), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("cancelled"));
        assert_eq!(unsafe { libc::kill(worker_pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}
