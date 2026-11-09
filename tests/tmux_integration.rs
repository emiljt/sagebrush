mod support;

use std::ffi::OsString;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use portable_pty::CommandBuilder;
use sagebrush::model::{Field, SessionId};
use sagebrush::tmux::TmuxClient;
use support::{IsolatedTmux, ProcessFixture, pty};

fn client(server: &IsolatedTmux, context: Option<OsString>) -> TmuxClient {
    let mut client = TmuxClient::new("tmux", context);
    client.env("TMUX_TMPDIR", server.directory_path());
    client
}

#[test]
fn absent_default_server_is_empty_and_is_not_created() {
    let server = IsolatedTmux::new().unwrap();
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(snapshot.sessions.is_empty());
    assert!(
        !server
            .command()
            .arg("has-session")
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn inherited_socket_selects_nondefault_server_without_guessing_active() {
    let server = IsolatedTmux::new().unwrap();
    server.start("default").unwrap();
    let socket = server.start_on_socket("custom").unwrap();
    let mut context = socket.into_os_string();
    context.push(",1,0");
    let snapshot = client(&server, Some(context))
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].name, Field::Known("custom".to_owned()));
    assert!(matches!(snapshot.sessions[0].active, Field::Unavailable(_)));
    assert!(!snapshot.issues.is_empty());
}

#[test]
fn session_name_is_preserved_including_whitespace_and_unicode() {
    let server = IsolatedTmux::new().unwrap();
    let name = " work-🌲 ";
    server.start(name).unwrap();
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(snapshot.sessions[0].name, Field::Known(name.to_owned()));
}

#[test]
fn shared_window_never_guesses_active_from_pane_membership() {
    let server = IsolatedTmux::new().unwrap();
    server.start("first").unwrap();
    for args in [
        ["new-session", "-d", "-s", "second", ""],
        ["link-window", "-s", "first:0", "-t", "second:1"],
    ] {
        let output = server
            .command()
            .args(args.into_iter().filter(|arg| !arg.is_empty()))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let socket = server
        .command()
        .args(["display-message", "-p", "#{socket_path}"])
        .output()
        .unwrap();
    assert!(socket.status.success());
    let socket = std::str::from_utf8(&socket.stdout).unwrap().trim_end();
    let context = OsString::from(format!("{socket},1,0"));
    let snapshot = client(&server, Some(context))
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(snapshot.sessions.len(), 2);
    assert!(
        snapshot
            .sessions
            .iter()
            .all(|session| matches!(session.active, Field::Unavailable(_)))
    );
}

#[test]
fn current_window_dimensions_match_server_values() {
    let server = IsolatedTmux::new().unwrap();
    server.start("sized").unwrap();
    let resize = server
        .command()
        .args(["resize-window", "-t", "sized:0", "-x", "91", "-y", "27"])
        .output()
        .unwrap();
    assert!(
        resize.status.success(),
        "{}",
        String::from_utf8_lossy(&resize.stderr)
    );
    let actual = server
        .command()
        .args([
            "display-message",
            "-p",
            "-t",
            "sized",
            "#{window_width}x#{window_height}",
        ])
        .output()
        .unwrap();
    assert!(actual.status.success());
    assert_eq!(String::from_utf8_lossy(&actual.stdout), "91x27\n");
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(snapshot.sessions[0].id, SessionId::parse("$0").unwrap());
    let Field::Known(window) = &snapshot.sessions[0].current_window else {
        panic!("current window must be available");
    };
    assert_eq!(window.width, Field::Known(91));
    assert_eq!(window.height, Field::Known(27));
}

#[test]
fn detached_sessions_are_read_without_mutating_the_server() {
    let server = IsolatedTmux::new().unwrap();
    server.start("first").unwrap();
    let created = server
        .command()
        .args(["new-session", "-d", "-s", "second"])
        .status()
        .unwrap();
    assert!(created.success());
    let before = server
        .command()
        .args([
            "list-sessions",
            "-F",
            "#{session_id}:#{session_name}:#{session_windows}",
        ])
        .output()
        .unwrap();
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    let after = server
        .command()
        .args([
            "list-sessions",
            "-F",
            "#{session_id}:#{session_name}:#{session_windows}",
        ])
        .output()
        .unwrap();
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(snapshot.sessions.len(), 2);
    assert_eq!(snapshot.sessions[0].name, Field::Known("first".to_owned()));
    assert_eq!(snapshot.sessions[1].name, Field::Known("second".to_owned()));
    for session in &snapshot.sessions {
        assert_eq!(session.active, Field::Known(false));
        assert_eq!(session.window_count, Field::Known(1));
        assert!(matches!(session.current_window, Field::Known(_)));
    }
}

#[test]
fn attached_and_detached_sessions_are_read_without_changing_client_state() {
    if std::env::var_os("SAGEBRUSH_ACTIVE_PROBE").is_some() {
        let snapshot = TmuxClient::from_environment()
            .acquire(&AtomicBool::new(false))
            .unwrap();
        let attached = snapshot
            .sessions
            .iter()
            .find(|session| session.name == Field::Known("attached".into()))
            .unwrap();
        let detached = snapshot
            .sessions
            .iter()
            .find(|session| session.name == Field::Known("detached".into()))
            .unwrap();
        assert_eq!(attached.active, Field::Known(true));
        assert_eq!(detached.active, Field::Known(false));
        return;
    }
    let server = IsolatedTmux::new().unwrap();
    server.start("attached").unwrap();
    assert!(
        server
            .command()
            .args(["new-session", "-d", "-s", "detached"])
            .status()
            .unwrap()
            .success()
    );
    let pair = pty(80, 24).unwrap();
    let mut command = CommandBuilder::new("tmux");
    command.args(["attach-session", "-t", "attached"]);
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env_remove("TMUX_PANE");
    command.env("TERM", "xterm");
    let mut child = pair.slave.spawn_command(command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let before = loop {
        let output = server
            .command()
            .args(["list-clients", "-F", "#{client_tty}:#{session_id}"])
            .output()
            .unwrap();
        assert!(output.status.success());
        if !output.stdout.is_empty() {
            break output.stdout;
        }
        assert!(Instant::now() < deadline, "tmux client did not attach");
        std::thread::sleep(Duration::from_millis(10));
    };
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    let after = server
        .command()
        .args(["list-clients", "-F", "#{client_tty}:#{session_id}"])
        .output()
        .unwrap();
    assert!(after.status.success());
    assert_eq!(before, after.stdout);
    assert_eq!(snapshot.sessions.len(), 2);
    assert!(
        snapshot
            .sessions
            .iter()
            .all(|session| session.active == Field::Known(false))
    );
    let socket = server
        .command()
        .args(["display-message", "-p", "#{socket_path}"])
        .output()
        .unwrap();
    assert!(socket.status.success());
    let context = format!(
        "{},1,0",
        std::str::from_utf8(&socket.stdout).unwrap().trim_end()
    );
    let tty = std::str::from_utf8(&before)
        .unwrap()
        .split_once(':')
        .unwrap()
        .0;
    let probe = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "attached_and_detached_sessions_are_read_without_changing_client_state",
        ])
        .env("SAGEBRUSH_ACTIVE_PROBE", "1")
        .env("TMUX_TMPDIR", server.directory_path())
        .env("TMUX", context)
        .stdin(std::fs::File::open(tty).unwrap())
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "active-context probe failed:\n{}\n{}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(
        server
            .command()
            .args(["detach-client", "-t", tty])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("tmux client did not exit after detach");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn invalid_inherited_context_never_falls_back_to_default() {
    let server = IsolatedTmux::new().unwrap();
    server.start("default").unwrap();
    let error = client(&server, Some(OsString::from("invalid")))
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("context"));
    let mut missing_socket = server.directory_path().join("missing").into_os_string();
    missing_socket.push(",1,0");
    let error = client(&server, Some(missing_socket))
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("server"));
}

#[test]
fn session_removed_during_a_real_query_is_not_returned() {
    let server = IsolatedTmux::new().unwrap();
    server.start("first").unwrap();
    assert!(
        server
            .command()
            .args(["new-session", "-d", "-s", "second"])
            .status()
            .unwrap()
            .success()
    );
    let wrapper = ProcessFixture::new(
        "#!/bin/sh\nshift\nif [ \"$1\" = display-message ] && [ \"$3\" = -t ] && [ \"$4\" = '$0' ]; then\n  tmux -u \"$@\" || exit $?\n  tmux -u kill-session -t \"$4\" || exit $?\n  exit 0\nfi\nexec tmux -u \"$@\"\n",
    )
    .unwrap();
    let mut client = TmuxClient::new(wrapper.path(), None);
    client.env("TMUX_TMPDIR", server.directory_path());
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].name, Field::Known("second".into()));
}

#[test]
fn acquisition_handles_one_hundred_sessions() {
    let server = IsolatedTmux::new().unwrap();
    server.start("session0").unwrap();
    for index in 1..100 {
        let output = server
            .command()
            .args([
                "new-session",
                "-d",
                "-s",
                &format!("session{index}"),
                "exec sleep 60",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let start = Instant::now();
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    eprintln!("100-session acquisition: {:?}", start.elapsed());
    assert_eq!(snapshot.sessions.len(), 100);
    assert!(snapshot.issues.is_empty());
}

#[test]
fn running_server_with_no_sessions_is_empty() {
    let server = IsolatedTmux::new().unwrap();
    server.start("temporary").unwrap();
    assert!(
        server
            .command()
            .args(["set-option", "-s", "exit-empty", "off"])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        server
            .command()
            .args(["kill-session", "-t", "temporary"])
            .status()
            .unwrap()
            .success()
    );
    let snapshot = client(&server, None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(snapshot.sessions.is_empty());
}
