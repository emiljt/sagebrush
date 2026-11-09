use std::ffi::OsStr;
use std::time::{Instant, UNIX_EPOCH};

use ratatui::{Terminal, backend::TestBackend};
use sagebrush::model::{
    CurrentWindow, Field, InvokingContext, Session, SessionId, SessionSnapshot, WindowId,
};
use sagebrush::tui::{state::AppState, view};

fn snapshot(partial: bool) -> SessionSnapshot {
    let rows = vec![
        Session {
            id: SessionId::parse("$1").unwrap(),
            name: Field::Known("alpha".into()),
            window_count: Field::Known(2),
            created_at: Field::Known(UNIX_EPOCH),
            current_window: Field::Known(CurrentWindow {
                id: WindowId::parse("@1").unwrap(),
                width: Field::Known(120),
                height: Field::Known(30),
            }),
            active: Field::Known(false),
        },
        Session {
            id: SessionId::parse("$2").unwrap(),
            name: if partial {
                Field::Unavailable("invalid name".into())
            } else {
                Field::Known("beta".into())
            },
            window_count: Field::Known(1),
            created_at: Field::Known(UNIX_EPOCH),
            current_window: Field::Known(CurrentWindow {
                id: WindowId::parse("@2").unwrap(),
                width: Field::Known(80),
                height: Field::Known(24),
            }),
            active: Field::Known(false),
        },
    ];
    SessionSnapshot::new(
        rows,
        UNIX_EPOCH,
        InvokingContext::InsideTmux {
            session_id: SessionId::parse("$2").unwrap(),
        },
    )
    .unwrap()
}

fn draw(state: &mut AppState, now: Instant) -> String {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view::render(frame, state, now, false))
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

#[test]
fn unavailable_current_window_shows_both_dimensions_in_row_and_crawl() {
    let now = Instant::now();
    let mut rows = snapshot(false).sessions;
    rows[0].current_window = Field::Unavailable("invalid current window".into());
    let snapshot = SessionSnapshot::new(rows, UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Ok(snapshot));
    let rendered = draw(&mut state, now);
    let row = rendered
        .lines()
        .find(|line| line.contains("alpha"))
        .unwrap();
    assert!(row.contains("-x-"), "{row}");
    assert!(state.crawl_text().unwrap().contains("Size: -x-"));
}

#[test]
fn populated_view_distinguishes_focus_and_active_and_shows_all_fields() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Ok(snapshot(false)));
    let rendered = draw(&mut state, now);
    assert!(rendered.contains("NAME"), "{rendered}");
    assert!(rendered.contains("WINDOWS"), "{rendered}");
    assert!(rendered.contains("UPTIME"), "{rendered}");
    assert!(rendered.contains("SIZE"), "{rendered}");
    assert!(rendered.contains("ACTIVE"), "{rendered}");
    assert!(
        rendered.contains("alpha") && rendered.contains("beta"),
        "{rendered}"
    );
    assert!(
        rendered.contains("120x30") && rendered.contains("80x24"),
        "{rendered}"
    );
    let alpha = rendered
        .lines()
        .find(|line| line.contains("alpha"))
        .unwrap();
    let beta = rendered.lines().find(|line| line.contains("beta")).unwrap();
    assert!(alpha.starts_with("> "), "{alpha}");
    assert!(beta.starts_with("  *"), "{beta}");
    assert!(rendered.contains("Name: alpha"), "{rendered}");
}

#[test]
fn loading_empty_partial_and_error_states_are_visible() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(draw(&mut state, now).contains("Loading sessions"));
    assert!(state.begin_due_request(now));
    state.finish_request(
        now,
        Ok(SessionSnapshot::new(vec![], UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap()),
    );
    assert!(draw(&mut state, now).contains("No tmux sessions found"));
    let next = now + std::time::Duration::from_secs(1);
    assert!(state.begin_due_request(next));
    state.finish_request(next, Ok(snapshot(true)));
    let rendered = draw(&mut state, next);
    assert!(rendered.contains("partial"), "{rendered}");
    assert!(rendered.contains("invalid name"), "{rendered}");
    let next = next + std::time::Duration::from_secs(1);
    assert!(state.begin_due_request(next));
    state.finish_request(next, Err("offline; check tmux".into()));
    let rendered = draw(&mut state, next);
    assert!(
        rendered.contains("stale") && rendered.contains("offline"),
        "{rendered}"
    );
    let retry = next + std::time::Duration::from_secs(5);
    assert!(state.begin_due_request(retry));
    let rendered = draw(&mut state, retry);
    assert!(
        rendered.contains("Refreshing")
            && rendered.contains("stale")
            && rendered.contains("offline"),
        "{rendered}"
    );
}

#[test]
fn help_describes_keys_refresh_and_crawl() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    state.handle_key(crossterm::event::KeyCode::Char('?'), now);
    let rendered = draw(&mut state, now);
    for word in [
        "j / k    Move list focus or scroll help.",
        "?        Open this help.",
        "Esc      Return to the session list.",
        "q        Quit and restore the terminal.",
        "* marks the invoking session; ? means active status is unavailable.",
        "Sessions refresh automatically.",
        "The selected session's complete details crawl across the screen.",
    ] {
        assert!(rendered.contains(word), "{word}: {rendered}");
    }
}

#[test]
fn unavailable_active_status_and_initial_error_are_visible() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Err("permission denied".into()));
    let rendered = draw(&mut state, now);
    assert!(
        rendered.contains("Failed") && rendered.contains("permission denied"),
        "{rendered}"
    );
    let next = now + std::time::Duration::from_secs(5);
    assert!(state.begin_due_request(next));
    let rows = snapshot(false).sessions;
    state.finish_request(
        next,
        Ok(SessionSnapshot::new(rows, UNIX_EPOCH, InvokingContext::ActiveUnavailable).unwrap()),
    );
    let rendered = draw(&mut state, next);
    let alpha = rendered
        .lines()
        .find(|line| line.contains("alpha"))
        .unwrap();
    assert!(alpha.starts_with("> ?"), "{alpha}");
    assert!(rendered.contains("cannot reliably identify"), "{rendered}");
}

#[test]
fn crawl_advances_by_one_cell_and_survives_refresh() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Ok(snapshot(false)));
    assert!(
        draw(&mut state, now)
            .lines()
            .nth(26)
            .unwrap()
            .starts_with("Name: alpha")
    );
    let next = now + std::time::Duration::from_secs(1);
    assert!(state.begin_due_request(next));
    assert!(draw(&mut state, next).contains("Refreshing"));
    let mut updated = snapshot(false);
    updated.sessions[0].name = Field::Known("renamed".into());
    state.finish_request(next, Ok(updated));
    let rendered = draw(&mut state, next);
    assert!(
        rendered.lines().nth(26).unwrap().starts_with("ame: alpha"),
        "{rendered}"
    );
}

#[test]
fn color_controls_apply_independently() {
    assert!(view::color_enabled(false, None, Some(OsStr::new("xterm"))));
    assert!(!view::color_enabled(true, None, Some(OsStr::new("xterm"))));
    assert!(!view::color_enabled(
        false,
        Some(OsStr::new("1")),
        Some(OsStr::new("xterm"))
    ));
    assert!(view::color_enabled(
        false,
        Some(OsStr::new("")),
        Some(OsStr::new("xterm"))
    ));
    assert!(!view::color_enabled(false, None, Some(OsStr::new("dumb"))));
}
