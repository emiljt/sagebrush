mod support;

use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::symlink;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

use sagebrush::model::{
    CurrentWindow, Field, InvokingContext, Session, SessionId, SessionSnapshot, WindowId,
};
use serde_json::Value;
use support::ProcessFixture;

fn fixture(script: &str) -> (ProcessFixture, tempfile::TempDir) {
    let fixture = ProcessFixture::new(script).unwrap();
    let directory = tempfile::tempdir().unwrap();
    symlink(fixture.path(), directory.path().join("tmux")).unwrap();
    (fixture, directory)
}

fn app(directory: &tempfile::TempDir, args: &[&str], partial: bool) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sagebrush"))
        .args(args)
        .env("PATH", directory.path())
        .env("BAD_WIDTH", if partial { "1" } else { "0" })
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .unwrap()
}

const TMUX: &str = "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) if [ \"$BAD_WIDTH\" = 1 ]; then width=0; else width=80; fi; printf '$10\\t1\\t0\\t@10\\t90\\t30\\n$2\\t2\\t0\\t@2\\t%s\\t24\\n' \"$width\";; display-message) if [ \"$3\" != -t ]; then echo '3.4'; elif [ \"$4\" = '$2' ]; then printf ' a\\tb\\nslash\\\\end\\n'; else echo later; fi;; esac\n";

#[test]
fn json_preserves_names_types_six_keys_and_numeric_order() {
    let (_fixture, directory) = fixture(TMUX);
    let output = app(&directory, &["list", "--json"], false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.len(), 2);
    let object = rows[0].as_object().unwrap();
    let keys: std::collections::BTreeSet<_> = object.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["active", "height", "name", "uptime", "width", "windows"]
            .into_iter()
            .collect()
    );
    assert_eq!(rows[0]["name"], " a\tb\nslash\\end");
    assert_eq!(rows[0]["windows"], 2);
    assert!(rows[0]["uptime"].as_u64().is_some());
    assert_eq!(rows[0]["width"], 80);
    assert_eq!(rows[0]["height"], 24);
    assert_eq!(rows[0]["active"], false);
    assert_eq!(rows[1]["name"], "later");
}

#[test]
fn json_and_plain_preserve_printable_unicode() {
    let (_fixture, directory) = fixture(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = -t ]; then printf '雪🌲\\n'; else echo '3.4'; fi;; esac\n",
    );
    let json = app(&directory, &["list", "--json"], false);
    assert_eq!(json.status.code(), Some(0), "{json:?}");
    let rows: Vec<Value> = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(rows[0]["name"], "雪🌲");
    let plain = app(&directory, &["list", "--plain"], false);
    assert_eq!(plain.status.code(), Some(0), "{plain:?}");
    assert!(
        String::from_utf8(plain.stdout)
            .unwrap()
            .starts_with("雪🌲\t")
    );
}

#[test]
fn json_emits_independent_nulls_and_partial_diagnostics() {
    let (_fixture, directory) = fixture(TMUX);
    let output = app(&directory, &["list", "--json"], true);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(rows[0]["width"].is_null());
    assert_eq!(rows[0]["height"], 24);
    assert_eq!(rows[0]["windows"], 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("width"));
}

#[test]
fn plain_records_are_headerless_five_field_and_control_safe() {
    let (_fixture, directory) = fixture(TMUX);
    let output = app(&directory, &["list", "--plain"], false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let rows: Vec<_> = stdout
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].len(), 5);
    assert_eq!(rows[0][0], " a\\tb\\nslash\\\\end");
    assert_eq!(rows[0][1], "2");
    assert_eq!(rows[0][3], "80x24");
    assert_eq!(rows[0][4], "no");
    assert_eq!(rows[1][0], "later");
}

#[test]
fn empty_results_use_format_specific_streams() {
    let (_fixture, directory) = fixture(
        "#!/bin/sh\nshift\nif [ \"$1\" = -V ]; then echo 'tmux 3.4'; else echo 'no server running on /tmp/example' >&2; exit 1; fi\n",
    );
    for (args, expected) in [
        (vec!["list", "--json"], "[]\n"),
        (vec!["list", "--plain"], ""),
        (vec!["list"], ""),
    ] {
        let output = app(&directory, &args, false);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn redirected_streams_and_color_controls_never_emit_ansi() {
    let (_fixture, directory) = fixture(TMUX);
    for (args, no_color, term) in [
        (vec!["list"], "", "xterm"),
        (vec!["list", "--no-color"], "", "xterm"),
        (vec!["--no-color", "list"], "", "xterm"),
        (vec!["list"], "1", "xterm"),
        (vec!["list"], "", "dumb"),
        (vec!["list", "--json"], "", "xterm"),
        (vec!["list", "--plain"], "", "xterm"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
            .args(args)
            .env("PATH", directory.path())
            .env("BAD_WIDTH", "1")
            .env("NO_COLOR", no_color)
            .env("TERM", term)
            .env_remove("TMUX")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(!output.stdout.contains(&0x1b));
        assert!(!output.stderr.contains(&0x1b));
    }
}

#[test]
fn color_suppression_respects_each_stream_independently() {
    let (_fixture, directory) = fixture(TMUX);
    for (no_color, term, flag) in [
        ("", "xterm", false),
        ("1", "xterm", false),
        ("", "dumb", false),
        ("", "xterm", true),
    ] {
        for tty_stdout in [true, false] {
            let mut master = -1;
            let mut slave = -1;
            assert_eq!(
                unsafe {
                    libc::openpty(
                        &mut master,
                        &mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                },
                0
            );
            let mut master = unsafe { std::fs::File::from_raw_fd(master) };
            let slave = unsafe { std::fs::File::from_raw_fd(slave) };
            assert_eq!(
                unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
                0
            );
            let mut command = Command::new(env!("CARGO_BIN_EXE_sagebrush"));
            command
                .arg("list")
                .env("PATH", directory.path())
                .env("BAD_WIDTH", "1")
                .env("NO_COLOR", no_color)
                .env("TERM", term)
                .env_remove("TMUX")
                .env_remove("TMUX_PANE");
            if flag {
                command.arg("--no-color");
            }
            if tty_stdout {
                command.stdout(Stdio::from(slave)).stderr(Stdio::piped());
            } else {
                command.stdout(Stdio::piped()).stderr(Stdio::from(slave));
            }
            let reader = std::thread::spawn(move || {
                let mut terminal_output = Vec::new();
                let mut buffer = [0; 1024];
                loop {
                    match master.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => terminal_output.extend_from_slice(&buffer[..count]),
                        Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
                        Err(error) => panic!("terminal read failed: {error}"),
                    }
                }
                terminal_output
            });
            let result = command.output().unwrap();
            drop(command);
            assert_eq!(result.status.code(), Some(1));
            let terminal_output = reader.join().unwrap();
            let (stdout, stderr) = if tty_stdout {
                (&terminal_output, &result.stderr)
            } else {
                (&result.stdout, &terminal_output)
            };
            assert!(!stdout.is_empty() && !stderr.is_empty());
            assert!(!stdout.contains(&0x1b) && !stderr.contains(&0x1b));
        }
    }
}

#[test]
fn unavailable_current_window_renders_both_dimensions() {
    let session = Session {
        id: SessionId::parse("$1").unwrap(),
        name: Field::Known("name".into()),
        window_count: Field::Known(1),
        created_at: Field::Known(UNIX_EPOCH),
        current_window: Field::Unavailable("invalid current window".into()),
        active: Field::Known(false),
    };
    let snapshot =
        SessionSnapshot::new(vec![session], UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap();
    assert!(sagebrush::output::plain(&snapshot).contains("\tunavailablexunavailable\t"));
    let json: Value = serde_json::from_str(&sagebrush::output::json(&snapshot).unwrap()).unwrap();
    assert!(json[0]["width"].is_null());
    assert!(json[0]["height"].is_null());
}

#[test]
fn json_nulls_each_unavailable_field_independently() {
    let session = Session {
        id: SessionId::parse("$1").unwrap(),
        name: Field::Known("name".into()),
        window_count: Field::Known(1),
        created_at: Field::Known(UNIX_EPOCH),
        current_window: Field::Known(CurrentWindow {
            id: WindowId::parse("@1").unwrap(),
            width: Field::Known(80),
            height: Field::Known(24),
        }),
        active: Field::Known(false),
    };
    for field in ["name", "windows", "uptime", "width", "height", "active"] {
        let mut row = session.clone();
        let context = if field == "active" {
            InvokingContext::ActiveUnavailable
        } else {
            InvokingContext::OutsideTmux
        };
        match field {
            "name" => row.name = Field::Unavailable("invalid name".into()),
            "windows" => row.window_count = Field::Unavailable("invalid count".into()),
            "uptime" => row.created_at = Field::Known(UNIX_EPOCH + Duration::from_secs(1)),
            "width" => {
                let Field::Known(window) = &mut row.current_window else {
                    unreachable!()
                };
                window.width = Field::Unavailable("invalid width".into());
            }
            "height" => {
                let Field::Known(window) = &mut row.current_window else {
                    unreachable!()
                };
                window.height = Field::Unavailable("invalid height".into());
            }
            _ => {}
        }
        let snapshot = SessionSnapshot::new(vec![row], UNIX_EPOCH, context).unwrap();
        let output = sagebrush::output::json(&snapshot).unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();
        for key in ["name", "windows", "uptime", "width", "height", "active"] {
            assert_eq!(value[0][key].is_null(), key == field, "{field}: {output}");
        }
    }
}

#[test]
fn fatal_failures_and_closed_pipes_have_consistent_format_statuses() {
    let (_fixture, directory) = fixture(TMUX);
    for args in [
        vec!["list"],
        vec!["list", "--json"],
        vec!["list", "--plain"],
    ] {
        let failed = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
            .args(&args)
            .env("PATH", "")
            .env_remove("TMUX")
            .output()
            .unwrap();
        assert_eq!(failed.status.code(), Some(1), "{args:?}: {failed:?}");
        assert!(failed.stdout.is_empty());
        assert!(!failed.stderr.is_empty());
        let mut child = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
            .args(&args)
            .env("PATH", directory.path())
            .env("BAD_WIDTH", "0")
            .env_remove("TMUX")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(141), "{args:?}: {output:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panic"));
    }
}

#[test]
fn signal_cancels_listing_while_the_output_pipe_is_blocked() {
    let (_fixture, directory) = fixture(
        "#!/bin/sh\nshift\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" != -t ]; then echo '3.4'; else /usr/bin/head -c 1048576 /dev/zero | /usr/bin/tr '\\000' x; printf '\\n'; : > \"$READY\"; fi;; esac\n",
    );
    let ready = directory.path().join("ready");
    let mut pipe = [-1; 2];
    assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
    let _reader = unsafe { std::fs::File::from_raw_fd(pipe[0]) };
    let writer = unsafe { std::fs::File::from_raw_fd(pipe[1]) };
    let original_flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(original_flags, -1);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sagebrush"))
        .args(["list", "--json"])
        .env("PATH", directory.path())
        .env("READY", &ready)
        .env_remove("TMUX")
        .stdout(Stdio::from(writer.try_clone().unwrap()))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while !ready.exists() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("large tmux response did not complete");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("signal did not interrupt blocked output");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.code(), Some(143));
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) },
        original_flags
    );
}

#[test]
fn closed_diagnostic_stream_returns_failure_without_panicking() {
    let (_fixture, directory) = fixture(TMUX);
    for partial in [true, false] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sagebrush"));
        command.args(["list", "--json"]).env_remove("TMUX");
        if partial {
            command.env("PATH", directory.path()).env("BAD_WIDTH", "1");
        } else {
            command.env("PATH", "");
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stderr.take());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
    }
}

#[test]
fn timed_out_json_query_leaves_stdout_empty() {
    let (_fixture, directory) = fixture("#!/bin/sh\nexec /bin/sleep 30\n");
    let started = Instant::now();
    let output = app(&directory, &["list", "--json"], false);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(8));
}
