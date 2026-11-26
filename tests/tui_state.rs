use std::time::{Duration, Instant, UNIX_EPOCH};

use crossterm::event::KeyCode;
use sagebrush::model::{
    CurrentWindow, Field, InvokingContext, Session, SessionId, SessionSnapshot, WindowId,
};
use sagebrush::tui::state::{AppState, HELP_LINES, LoadState, Screen, wrapped_help};

fn snapshot(rows: &[(&str, &str)], partial: bool) -> SessionSnapshot {
    let sessions = rows
        .iter()
        .map(|(id, name)| Session {
            id: SessionId::parse(id).unwrap(),
            name: if partial {
                Field::Unavailable("invalid name".into())
            } else {
                Field::Known((*name).into())
            },
            window_count: Field::Known(1),
            created_at: Field::Known(UNIX_EPOCH),
            current_window: Field::Known(CurrentWindow {
                id: WindowId::parse("@1").unwrap(),
                width: Field::Known(80),
                height: Field::Known(24),
            }),
            active: Field::Known(false),
        })
        .collect();
    SessionSnapshot::new(sessions, UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap()
}

fn load(state: &mut AppState, now: Instant, result: Result<SessionSnapshot, String>) {
    assert!(state.begin_due_request(now));
    state.finish_request(now, result);
}

#[test]
fn navigation_and_help_do_not_change_active_status() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    load(
        &mut state,
        now,
        Ok(snapshot(&[("$1", "one"), ("$2", "two")], false)),
    );
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$1");
    assert!(!state.handle_key(KeyCode::Char('j'), now));
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$2");
    state.handle_key(KeyCode::Char('?'), now);
    assert_eq!(state.screen, Screen::Help);
    state.handle_key(KeyCode::Char('j'), now);
    assert_eq!(state.help_scroll, 1);
    state.handle_key(KeyCode::Char('k'), now);
    assert_eq!(state.help_scroll, 0);
    state.handle_key(KeyCode::Esc, now);
    assert_eq!(state.screen, Screen::List);
    state.handle_key(KeyCode::Char('x'), now);
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$2");
    assert!(state.handle_key(KeyCode::Char('q'), now));
    assert_eq!(
        state.snapshot.as_ref().unwrap().sessions[1].active,
        Field::Known(false)
    );
}

#[test]
fn focus_survives_rename_and_moves_to_numeric_successor_or_predecessor() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    load(
        &mut state,
        now,
        Ok(snapshot(
            &[("$1", "one"), ("$2", "two"), ("$10", "ten")],
            false,
        )),
    );
    state.handle_key(KeyCode::Char('j'), now);
    let next = now + Duration::from_secs(1);
    load(
        &mut state,
        next,
        Ok(snapshot(
            &[("$1", "one"), ("$2", "renamed"), ("$10", "ten")],
            false,
        )),
    );
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$2");
    let next = next + Duration::from_secs(1);
    load(
        &mut state,
        next,
        Ok(snapshot(&[("$1", "one"), ("$10", "ten")], false)),
    );
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$10");
    let next = next + Duration::from_secs(1);
    load(&mut state, next, Ok(snapshot(&[("$1", "one")], false)));
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$1");
    let next = next + Duration::from_secs(1);
    load(&mut state, next, Ok(snapshot(&[], false)));
    assert!(state.focus.is_none());
}

#[test]
fn requests_do_not_overlap_and_failures_back_off_after_completion() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert_eq!(state.load_state, LoadState::Loading);
    assert!(state.begin_due_request(now));
    assert!(!state.begin_due_request(now + Duration::from_secs(10)));
    let finished = now + Duration::from_secs(2);
    state.finish_request(finished, Err("offline".into()));
    assert!(matches!(state.load_state, LoadState::Failed(_)));
    assert!(!state.begin_due_request(finished + Duration::from_secs(4)));
    assert!(state.begin_due_request(finished + Duration::from_secs(5)));
    state.finish_request(
        finished + Duration::from_secs(5),
        Ok(snapshot(&[("$1", "one")], false)),
    );
    assert_eq!(state.load_state, LoadState::Fresh);
    assert!(!state.begin_due_request(finished + Duration::from_secs(5)));
    assert!(state.begin_due_request(finished + Duration::from_secs(6)));
    assert_eq!(state.load_state, LoadState::Refreshing);
    state.finish_request(
        finished + Duration::from_secs(6),
        Err("lost connection".into()),
    );
    assert!(matches!(state.load_state, LoadState::Stale(_)));
    assert_eq!(state.snapshot.as_ref().unwrap().sessions.len(), 1);
}

#[test]
fn partial_refresh_retries_regularly_and_warning_survives_old_crawl() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    load(&mut state, now, Ok(snapshot(&[("$1", "one")], true)));
    assert_eq!(state.load_state, LoadState::Partial);
    assert!(state.partial_warning());
    assert!(state.crawl_text().unwrap().contains("invalid name"));
    let next = now + Duration::from_secs(1);
    load(&mut state, next, Ok(snapshot(&[("$1", "updated")], false)));
    assert_eq!(state.load_state, LoadState::Fresh);
    assert!(state.partial_warning());
    state.advance_crawl(now + Duration::from_secs(1000));
    assert!(!state.partial_warning());
}

#[test]
fn crawl_keeps_values_until_next_pass_and_restarts_on_focus_change() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    load(
        &mut state,
        now,
        Ok(snapshot(&[("$1", "before"), ("$2", "second")], false)),
    );
    assert!(state.crawl_text().unwrap().contains("before"));
    let next = now + Duration::from_secs(1);
    load(
        &mut state,
        next,
        Ok(snapshot(&[("$1", "after"), ("$2", "second")], false)),
    );
    assert!(state.crawl_text().unwrap().contains("before"));
    state.advance_crawl(now + Duration::from_secs(1000));
    assert!(state.crawl_text().unwrap().contains("after"));
    state.handle_key(KeyCode::Char('j'), now + Duration::from_secs(1001));
    assert!(state.crawl_text().unwrap().contains("second"));
}

#[test]
fn navigation_and_resize_keep_focused_row_visible() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    state.resize(80, 10);
    load(
        &mut state,
        now,
        Ok(snapshot(
            &[
                ("$1", "one"),
                ("$2", "two"),
                ("$3", "three"),
                ("$4", "four"),
                ("$5", "five"),
            ],
            false,
        )),
    );
    for _ in 0..4 {
        state.handle_key(KeyCode::Char('j'), now);
    }
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$5");
    assert!(state.list_scroll > 0);
    state.resize(80, 30);
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$5");
    assert_eq!(state.list_scroll, 0);
    assert_eq!(state.viewport, (80, 30));
}

#[test]
fn completed_request_keeps_regular_refresh_alignment_after_skipped_ticks() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(
        now + Duration::from_millis(2500),
        Ok(snapshot(&[("$1", "one")], false)),
    );
    assert!(!state.begin_due_request(now + Duration::from_millis(2999)));
    assert!(state.begin_due_request(now + Duration::from_secs(3)));
}

#[test]
fn empty_results_are_fresh_and_help_scrolling_is_bounded() {
    let now = Instant::now();
    let mut state = AppState::new(now);
    state.resize(80, 10);
    load(&mut state, now, Ok(snapshot(&[], false)));
    assert_eq!(state.load_state, LoadState::Fresh);
    assert!(state.focus.is_none());
    state.handle_key(KeyCode::Char('?'), now);
    for _ in 0..100 {
        state.handle_key(KeyCode::Char('j'), now);
    }

    let end = state.help_scroll;
    state.handle_key(KeyCode::Char('j'), now);
    assert_eq!(state.help_scroll, end);
    state.resize(80, 100);
    assert_eq!(state.help_scroll, 0);
}

#[test]
fn narrow_help_wraps_without_losing_text_and_remains_scrollable() {
    let lines = wrapped_help(40);
    assert!(lines.len() > HELP_LINES.len());
    assert_eq!(lines.concat(), HELP_LINES.concat());
    assert!(
        lines
            .iter()
            .all(|line| unicode_width::UnicodeWidthStr::width(line.as_str()) <= 40)
    );
    let now = Instant::now();
    let mut state = AppState::new(now);
    state.resize(40, 10);
    state.handle_key(KeyCode::Char('?'), now);
    for _ in 0..100 {
        state.handle_key(KeyCode::Char('j'), now);
    }
    assert_eq!(state.help_scroll, lines.len() - 8);
}
