mod support;

use std::ffi::OsString;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use sagebrush::model::Field;
use sagebrush::tmux::{ProcessError, TmuxClient, run_process, verify_version};
use support::ProcessFixture;

fn tmux_fixture(script: &str) -> ProcessFixture {
    let script = script.replacen("#!/bin/sh\n", "#!/bin/sh\nshift\n", 1);
    ProcessFixture::new(&script).unwrap()
}

#[test]
fn versions_require_final_3_3_or_later() {
    for version in ["tmux 3.3", "3.3a", "3.4a", "3.10", "4.0"] {
        assert!(verify_version(version, "client").is_ok(), "{version}");
    }
    for version in ["3.2", "3.3-rc1", "3.3rc", "not a version", ""] {
        assert!(verify_version(version, "server").is_err(), "{version}");
    }
}

#[test]
fn process_reads_both_pipes_without_truncating() {
    let fixture =
        ProcessFixture::new("#!/bin/sh\nhead -c 131072 /dev/zero\nhead -c 131072 /dev/zero >&2\n")
            .unwrap();
    let output = run_process(
        fixture.command(),
        Instant::now() + Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 131_072);
    assert_eq!(output.stderr.len(), 131_072);
}

#[test]
fn resource_exhaustion_is_reported_as_an_error() {
    const CHILD: &str = "SAGEBRUSH_RLIMIT_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
            0
        );
        let mut limit = unsafe { limit.assume_init() };
        limit.rlim_cur = limit.rlim_cur.min(32);
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
        let mut files = Vec::new();
        while let Ok(file) = std::fs::File::open("/dev/null") {
            files.push(file);
        }
        let result = run_process(
            std::process::Command::new("/bin/true"),
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert!(
            matches!(result, Err(ProcessError::Io(ref error)) if error.raw_os_error() == Some(libc::EMFILE)),
            "{result:?}"
        );
        drop(files);
        return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "resource_exhaustion_is_reported_as_an_error"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn blocked_process_is_cancelled_at_deadline() {
    let fixture = ProcessFixture::new("#!/bin/sh\nexec sleep 5\n").unwrap();
    let start = Instant::now();
    let result = run_process(
        fixture.command(),
        start + Duration::from_millis(100),
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(ProcessError::Timeout)), "{result:?}");
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn cancellation_stops_a_blocked_process() {
    let fixture = ProcessFixture::new("#!/bin/sh\nexec sleep 5\n").unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    let signal = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        flag.store(true, Ordering::SeqCst);
    });
    let start = Instant::now();
    let result = run_process(
        fixture.command(),
        start + Duration::from_secs(5),
        &cancelled,
    );
    signal.join().unwrap();
    assert!(matches!(result, Err(ProcessError::Cancelled)));
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn separate_client_and_server_versions_are_checked() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; display-message) echo '3.2';; *) exit 99;; esac\n",
    );
    let error = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("server"), "{error}");
}

#[test]
fn invalid_client_version_stops_before_server_access() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.3-rc1';; *) : > \"$STATE\"; exit 99;; esac\n",
    );
    let marker = fixture.path().with_file_name("server-access");
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", &marker);
    let error = client.acquire(&AtomicBool::new(false)).unwrap_err();
    assert!(error.to_string().contains("client"), "{error}");
    assert!(!marker.exists());
}

#[test]
fn inherited_socket_is_passed_as_one_argument_without_fallback() {
    let fixture = tmux_fixture(
        "#!/bin/sh\nif [ \"$1\" = -V ]; then echo 'tmux 3.4'; exit 0; fi\n[ \"$1\" = -S ] && [ \"$2\" = \"$SOCKET\" ] || { echo 'wrong socket' >&2; exit 1; }\nshift 2\ncase \"$1\" in display-message) echo '3.4';; list-sessions) :;; *) exit 99;; esac\n",
    );
    let socket = fixture.path().with_file_name("socket, with spaces");
    let mut context = OsString::from(&socket);
    context.push(",123,0");
    let mut client = TmuxClient::new(fixture.path(), Some(context));
    client.env("SOCKET", &socket);
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert!(snapshot.sessions.is_empty());
}

#[test]
fn one_deadline_covers_client_and_server_queries() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) sleep 3; echo 'tmux 3.4';; display-message) exec sleep 3;; *) exit 99;; esac\n",
    );
    let start = Instant::now();
    let error = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error}");
    assert!(start.elapsed() >= Duration::from_secs(5));
    assert!(start.elapsed() < Duration::from_secs(7));
}

#[test]
fn missing_client_and_protocol_errors_are_reported() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; *) echo 'protocol version mismatch' >&2; exit 1;; esac\n",
    );
    let error = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("protocol version mismatch"));
    let error = TmuxClient::new(fixture.path().with_file_name("missing"), None)
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("tmux"));
}

#[test]
fn access_failures_and_malformed_metadata_are_not_empty_successes() {
    for (script, message) in [
        (
            "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; *) echo 'permission denied' >&2; exit 1;; esac\n",
            "permission denied",
        ),
        (
            "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; display-message) echo '3.4';; list-sessions) printf 'bad record\\n';; esac\n",
            "framing",
        ),
    ] {
        let fixture = tmux_fixture(script);
        let error = TmuxClient::new(fixture.path(), None)
            .acquire(&AtomicBool::new(false))
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn absent_default_server_is_empty_but_inherited_server_failure_is_fatal() {
    let fixture = tmux_fixture(
        "#!/bin/sh\nif [ \"$1\" = -V ]; then echo 'tmux 3.4'; else echo 'no server running on /tmp/example' >&2; exit 1; fi\n",
    );
    let empty = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(empty.sessions.is_empty());
    let error = TmuxClient::new(fixture.path(), Some("/tmp/example,1,0".into()))
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("server"), "{error}");
}

#[test]
fn ambiguous_inherited_active_status_is_unavailable_with_a_diagnostic() {
    let fixture = tmux_fixture(
        "#!/bin/sh\nif [ \"$1\" = -V ]; then echo 'tmux 3.4'; exit 0; fi\n[ \"$1\" = -S ] || exit 99\nshift 2\ncase \"$1\" in display-message) if [ \"$3\" = -t ]; then echo name; else echo '3.4'; fi;; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; esac\n",
    );
    let snapshot = TmuxClient::new(fixture.path(), Some("/tmp/example,1,0".into()))
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(matches!(snapshot.sessions[0].active, Field::Unavailable(_)));
    assert!(
        snapshot
            .issues
            .iter()
            .any(|issue| issue.field == "active" && issue.message.contains("client"))
    );
}

#[test]
fn inherited_pipe_from_descendant_is_cancelled_at_deadline() {
    let fixture = ProcessFixture::new("#!/bin/sh\nsleep 5 &\nexit 0\n").unwrap();
    let start = Instant::now();
    let result = run_process(
        fixture.command(),
        start + Duration::from_millis(100),
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(ProcessError::Timeout)), "{result:?}");
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn metadata_fields_are_independent_and_names_are_not_trimmed() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t0\\t24\\n';; display-message) if [ \"$3\" = '-t' ]; then printf ' a\\tb\\\\c\\nend \\n'; else echo '3.4'; fi;; esac\n",
    );
    let snapshot = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    let session = &snapshot.sessions[0];
    assert_eq!(session.name, Field::Known(" a\tb\\c\nend ".to_owned()));
    let Field::Known(window) = &session.current_window else {
        panic!("window must be identifiable");
    };
    assert!(matches!(window.width, Field::Unavailable(_)));
    assert_eq!(window.height, Field::Known(24));
    assert_eq!(snapshot.issues.len(), 1);
}

#[test]
fn invalid_name_bytes_are_partial_but_duplicate_ids_are_fatal() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = '-t' ]; then printf '\\377\\n'; else echo '3.4'; fi;; esac\n",
    );
    let snapshot = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(matches!(snapshot.sessions[0].name, Field::Unavailable(_)));
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = '-t' ]; then echo name; else echo '3.4'; fi;; esac\n",
    );
    let error = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn disappearing_session_is_removed_after_reconciliation() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) if [ -e \"$STATE\" ]; then printf '$2\\t1\\t0\\t@2\\t80\\t24\\n'; else printf '$1\\t1\\t0\\t@1\\t80\\t24\\n$2\\t1\\t0\\t@2\\t80\\t24\\n'; fi;; display-message) if [ \"$3\" != '-t' ]; then echo '3.4'; elif [ \"$4\" = '$1' ]; then : > \"$STATE\"; echo 'session disappeared' >&2; exit 1; else echo remaining; fi;; esac\n",
    );
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", fixture.path().with_file_name("state"));
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].id.as_str(), "$2");
    assert_eq!(
        snapshot.sessions[0].name,
        Field::Known("remaining".to_owned())
    );
}

#[test]
fn session_disappearing_after_successful_name_lookup_is_reconciled() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) if [ -e \"$STATE\" ]; then printf '$2\\t1\\t0\\t@2\\t80\\t24\\n'; else printf '$1\\t1\\t0\\t@1\\t80\\t24\\n$2\\t1\\t0\\t@2\\t80\\t24\\n'; fi;; display-message) if [ \"$3\" != '-t' ]; then echo '3.4'; elif [ \"$4\" = '$1' ]; then echo removed; else : > \"$STATE\"; echo remaining; fi;; esac\n",
    );
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", fixture.path().with_file_name("state"));
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].id.as_str(), "$2");
    assert_eq!(snapshot.sessions[0].name, Field::Known("remaining".into()));
}

#[test]
fn new_session_during_name_lookup_is_reconciled() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n'; if [ -e \"$STATE\" ]; then printf '$2\\t1\\t0\\t@2\\t80\\t24\\n'; fi;; display-message) if [ \"$3\" != '-t' ]; then echo '3.4'; else : > \"$STATE\"; printf 'name%s\\n' \"$4\"; fi;; esac\n",
    );
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", fixture.path().with_file_name("state"));
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        snapshot
            .sessions
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        ["$1", "$2"]
    );
}

#[test]
fn metadata_reordering_during_name_lookup_does_not_cause_retries() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) if [ -e \"$STATE\" ]; then printf '$2\\t1\\t0\\t@2\\t80\\t24\\n$1\\t1\\t0\\t@1\\t80\\t24\\n'; else printf '$1\\t1\\t0\\t@1\\t80\\t24\\n$2\\t1\\t0\\t@2\\t80\\t24\\n'; fi;; display-message) if [ \"$3\" != '-t' ]; then echo '3.4'; else : > \"$STATE\"; printf 'name%s\\n' \"$4\"; fi;; esac\n",
    );
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", fixture.path().with_file_name("state"));
    let snapshot = client.acquire(&AtomicBool::new(false)).unwrap();
    assert_eq!(
        snapshot
            .sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        ["$1", "$2"]
    );
}

#[test]
fn duplicate_ids_during_reconciliation_are_fatal() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n'; if [ -e \"$STATE\" ]; then printf '$1\\t1\\t0\\t@1\\t80\\t24\\n'; fi;; display-message) if [ \"$3\" != '-t' ]; then echo '3.4'; else : > \"$STATE\"; echo name; fi;; esac\n",
    );
    let mut client = TmuxClient::new(fixture.path(), None);
    client.env("STATE", fixture.path().with_file_name("state"));
    let error = client.acquire(&AtomicBool::new(false)).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
}

#[test]
fn surviving_session_with_failed_name_query_is_partial() {
    let fixture = tmux_fixture(
        "#!/bin/sh\ncase \"$1\" in -V) echo 'tmux 3.4';; list-sessions) printf '$1\\t1\\t0\\t@1\\t80\\t24\\n';; display-message) if [ \"$3\" = '-t' ]; then echo 'name query failed' >&2; exit 1; else echo '3.4'; fi;; esac\n",
    );
    let snapshot = TmuxClient::new(fixture.path(), None)
        .acquire(&AtomicBool::new(false))
        .unwrap();
    assert!(matches!(snapshot.sessions[0].name, Field::Unavailable(_)));
    assert_eq!(snapshot.sessions[0].window_count, Field::Known(1));
    assert!(snapshot.issues[0].message.contains("name query failed"));
}

#[test]
fn timed_out_child_is_reaped() {
    let fixture =
        ProcessFixture::new("#!/bin/sh\necho $$ > \"$PID_FILE\"\nexec sleep 5\n").unwrap();
    let pid_path = fixture.path().with_file_name("pid");
    let mut command = fixture.command();
    command.env("PID_FILE", &pid_path);
    let result = run_process(
        command,
        Instant::now() + Duration::from_millis(100),
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(ProcessError::Timeout)), "{result:?}");
    let pid = std::fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
