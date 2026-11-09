mod support;

use std::time::{Duration, UNIX_EPOCH};

use support::{IsolatedTmux, ProcessFixture, TestClock, pty};

#[test]
fn clock_is_shared_and_controllable() {
    let clock = TestClock::new(UNIX_EPOCH);
    let other = clock.clone();
    clock.advance(Duration::from_secs(3));
    assert_eq!(other.now(), UNIX_EPOCH + Duration::from_secs(3));
}

#[test]
fn isolated_fixtures_use_distinct_tmux_directories() {
    let first = IsolatedTmux::new().unwrap();
    let second = IsolatedTmux::new().unwrap();
    assert_ne!(first.directory_path(), second.directory_path());
    let command = first.command();
    let vars: Vec<_> = command.get_envs().collect();
    assert!(
        vars.iter()
            .any(|(key, value)| *key == "TMUX_TMPDIR" && value.is_some())
    );
    assert!(
        vars.iter()
            .any(|(key, value)| *key == "TMUX" && value.is_none())
    );
}

#[test]
fn process_fixture_accepts_arguments_without_shell_interpolation() {
    let fixture = ProcessFixture::new("#!/bin/sh\nprintf '%s' \"$1\"\n").unwrap();
    let output = fixture.command().arg("a; b").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"a; b");
}

#[test]
fn isolated_tmux_can_start_and_stop() {
    let fixture = IsolatedTmux::new().unwrap();
    fixture.start("fixture").unwrap();
    let output = fixture.command().arg("list-sessions").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("fixture")
    );
    fixture.stop().unwrap();
}

#[test]
fn pty_uses_requested_dimensions() {
    let pair = pty(80, 24).unwrap();
    let size = pair.master.get_size().unwrap();
    assert_eq!((size.cols, size.rows), (80, 24));
}
