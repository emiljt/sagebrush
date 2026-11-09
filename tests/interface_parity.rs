mod support;

use std::ffi::OsString;
use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use portable_pty::CommandBuilder;
use ratatui::{Terminal, backend::TestBackend};
use sagebrush::model::{Field, SessionSnapshot, format_uptime, uptime};
use sagebrush::tmux::TmuxClient;
use sagebrush::tui::{state::AppState, view};
use serde_json::Value;
use support::{IsolatedTmux, pty};

fn render(snapshot: SessionSnapshot) -> String {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Ok(snapshot));
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| view::render(frame, &mut state, now, false))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..30)
        .map(|y| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compare(snapshot: SessionSnapshot, json: &Value, active: Option<bool>) {
    assert_eq!(snapshot.sessions.len(), 1);
    let row = &snapshot.sessions[0];
    let Field::Known(name) = &row.name else {
        panic!("name unavailable")
    };
    let Field::Known(windows) = row.window_count else {
        panic!("count unavailable")
    };
    let Field::Known(window) = &row.current_window else {
        panic!("window unavailable")
    };
    let (Field::Known(width), Field::Known(height)) = (&window.width, &window.height) else {
        panic!("dimensions unavailable");
    };
    let Field::Known(seconds) = uptime(&row.created_at, snapshot.captured_at) else {
        panic!("uptime unavailable");
    };
    assert_eq!(json["name"], *name);
    assert_eq!(json["windows"], windows);
    assert_eq!(json["width"], *width);
    assert_eq!(json["height"], *height);
    assert!(json["uptime"].as_u64().unwrap().abs_diff(seconds) <= 1);
    match active {
        None => {
            assert!(json["active"].is_null());
            assert!(matches!(row.active, Field::Unavailable(_)));
        }
        Some(active) => {
            assert_eq!(json["active"], active);
            assert_eq!(row.active, Field::Known(active));
        }
    }
    let rendered = render(snapshot.clone());
    let line = rendered.lines().find(|line| line.contains(name)).unwrap();
    assert!(line.contains(&format!("{width}x{height}")), "{line}");
    assert!(line.contains(&format_uptime(seconds)), "{line}");
    assert!(line.contains(&windows.to_string()), "{line}");
    let prefix = match active {
        None => "> ?",
        Some(true) => "> *",
        Some(false) => ">  ",
    };
    assert!(line.starts_with(prefix), "{line}");
}

#[test]
fn cli_and_tui_share_default_and_inherited_server_scope_without_mutation() {
    let server = IsolatedTmux::new().unwrap();
    server.start("default-session").unwrap();
    let socket = server.start_on_socket("custom-session").unwrap();
    let mut context: OsString = socket.into_os_string();
    context.push(",1,0");
    for (context, expected_name, status) in [
        (None, "default-session", 0),
        (Some(context), "custom-session", 1),
    ] {
        let mut tmux = TmuxClient::new("tmux", context.clone());
        tmux.env("TMUX_TMPDIR", server.directory_path());
        let before = tmux.acquire(&AtomicBool::new(false)).unwrap();
        let mut command = server.application(std::path::Path::new(env!("CARGO_BIN_EXE_sagebrush")));
        command.args(["list", "--json"]);
        if let Some(context) = &context {
            command.env("TMUX", context);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(status), "{output:?}");
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json[0]["name"], expected_name);
        compare(
            before.clone(),
            &json[0],
            if context.is_some() { None } else { Some(false) },
        );
        let after = tmux.acquire(&AtomicBool::new(false)).unwrap();
        assert_eq!(before.sessions, after.sessions);
    }
}

#[test]
fn cli_and_tui_agree_when_an_invoking_client_is_identifiable() {
    if std::env::var_os("SAGEBRUSH_PARITY_PROBE").is_some() {
        let tmux = TmuxClient::from_environment();
        let before = tmux.acquire(&AtomicBool::new(false)).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sagebrush"))
            .args(["list", "--json"])
            .stdin(std::process::Stdio::inherit())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        compare(before.clone(), &json[0], Some(true));
        let after = tmux.acquire(&AtomicBool::new(false)).unwrap();
        assert_eq!(before.sessions, after.sessions);
        return;
    }

    let server = IsolatedTmux::new().unwrap();
    server.start("attached").unwrap();
    let pair = pty(120, 30).unwrap();
    let mut command = CommandBuilder::new("tmux");
    command.args(["attach-session", "-t", "attached"]);
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env("TERM", "xterm");
    let mut client = pair.slave.spawn_command(command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let tty = loop {
        let output = server
            .command()
            .args(["list-clients", "-F", "#{client_tty}"])
            .output()
            .unwrap();
        assert!(output.status.success());
        if !output.stdout.is_empty() {
            break String::from_utf8(output.stdout)
                .unwrap()
                .trim_end()
                .to_owned();
        }
        assert!(Instant::now() < deadline, "client did not attach");
        std::thread::sleep(Duration::from_millis(10));
    };
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
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cli_and_tui_agree_when_an_invoking_client_is_identifiable",
        ])
        .env("SAGEBRUSH_PARITY_PROBE", "1")
        .env("TMUX_TMPDIR", server.directory_path())
        .env("TMUX", context)
        .stdin(std::fs::File::open(&tty).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        server
            .command()
            .args(["detach-client", "-t", &tty])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = client.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            client.kill().unwrap();
            client.wait().unwrap();
            panic!("tmux client did not detach");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn cli_and_tui_in_attached_and_shared_panes_do_not_guess_active_client() {
    let server = IsolatedTmux::new().unwrap();
    server.start("attached").unwrap();
    assert!(
        server
            .command()
            .args(["respawn-pane", "-k", "-t", "attached:0", "/bin/sh"])
            .status()
            .unwrap()
            .success()
    );
    let pair = pty(120, 30).unwrap();
    let mut command = CommandBuilder::new("tmux");
    command.args(["attach-session", "-t", "attached"]);
    command.env("TMUX_TMPDIR", server.directory_path());
    command.env_remove("TMUX");
    command.env_remove("TMUX_PANE");
    command.env("TERM", "xterm");
    let mut client = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().unwrap();
    let drain = std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let tty = loop {
        let output = server
            .command()
            .args(["list-clients", "-F", "#{client_tty}"])
            .output()
            .unwrap();
        if output.status.success() && !output.stdout.is_empty() {
            break String::from_utf8(output.stdout).unwrap().trim().to_owned();
        }
        assert!(Instant::now() < deadline, "client did not attach");
        std::thread::sleep(Duration::from_millis(10));
    };

    let files = tempfile::tempdir().unwrap();
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let app = quote(env!("CARGO_BIN_EXE_sagebrush"));
    for shared in [false, true] {
        if shared {
            assert!(
                server
                    .command()
                    .args(["new-session", "-d", "-s", "shared"])
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                server
                    .command()
                    .args(["link-window", "-s", "attached:0", "-t", "shared:1"])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let label = if shared { "shared" } else { "single" };
        let output = files.path().join(format!("{label}.json"));
        let errors = files.path().join(format!("{label}.err"));
        let status = files.path().join(format!("{label}.status"));
        let command = format!(
            "{app} list --json >{} 2>{}; printf '%s' \"$?\" >{}",
            quote(output.to_str().unwrap()),
            quote(errors.to_str().unwrap()),
            quote(status.to_str().unwrap())
        );
        assert!(
            server
                .command()
                .args(["send-keys", "-t", "attached:0", "-l", &command])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            server
                .command()
                .args(["send-keys", "-t", "attached:0", "Enter"])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let exit_status = loop {
            if let Ok(value) = std::fs::read_to_string(&status)
                && !value.is_empty()
            {
                break value;
            }
            if Instant::now() >= deadline {
                let screen = server
                    .command()
                    .args(["capture-pane", "-p", "-t", "attached:0"])
                    .output()
                    .unwrap();
                panic!(
                    "pane-launched CLI did not finish: {}",
                    String::from_utf8_lossy(&screen.stdout)
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(exit_status, "1");
        let json: Value = serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        let rows = json.as_array().unwrap();
        assert_eq!(rows.len(), if shared { 2 } else { 1 });
        assert!(rows.iter().all(|row| row["active"].is_null()));
        assert!(
            std::fs::read_to_string(errors)
                .unwrap()
                .contains("cannot reliably identify")
        );

        assert!(
            server
                .command()
                .args(["send-keys", "-t", "attached:0", "-l", &format!("{app} tui")])
                .status()
                .unwrap()
                .success()
        );
        assert!(
            server
                .command()
                .args(["send-keys", "-t", "attached:0", "Enter"])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let screen = server
                .command()
                .args(["capture-pane", "-p", "-t", "attached:0"])
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&screen.stdout);
            if text
                .lines()
                .any(|line| line.starts_with("> ?") && line.contains("attached"))
            {
                for (index, row) in rows.iter().enumerate() {
                    let name = row["name"].as_str().unwrap();
                    let line = text.lines().find(|line| line.contains(name)).unwrap();
                    assert!(
                        line.starts_with(if index == 0 { "> ?" } else { "  ?" }),
                        "{line}"
                    );
                    assert!(line.contains(&row["windows"].to_string()), "{line}");
                    assert!(
                        line.contains(&format!("{}x{}", row["width"], row["height"])),
                        "{line}"
                    );
                }
                break;
            }
            assert!(
                Instant::now() < deadline,
                "pane-launched TUI did not show unavailable active status: {text}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            server
                .command()
                .args(["send-keys", "-t", "attached:0", "q"])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let pane = server
                .command()
                .args([
                    "display-message",
                    "-p",
                    "-t",
                    "attached:0",
                    "#{pane_current_command}",
                ])
                .output()
                .unwrap();
            if String::from_utf8_lossy(&pane.stdout).trim() != "sagebrush" {
                break;
            }
            assert!(Instant::now() < deadline, "pane-launched TUI did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    assert!(
        server
            .command()
            .args(["detach-client", "-t", &tty])
            .status()
            .unwrap()
            .success()
    );
    client.wait().unwrap();
    drop(pair.master);
    drain.join().unwrap();
}
