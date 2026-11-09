use std::time::{Duration, Instant, UNIX_EPOCH};

use ratatui::{Terminal, backend::TestBackend};
use sagebrush::model::{
    CurrentWindow, Field, InvokingContext, Session, SessionId, SessionSnapshot, WindowId,
};
use sagebrush::tui::{state::AppState, view};

fn build_state(name: &str, now: Instant) -> AppState {
    let session = Session {
        id: SessionId::parse("$1").unwrap(),
        name: Field::Known(name.into()),
        window_count: Field::Known(7),
        created_at: Field::Known(UNIX_EPOCH),
        current_window: Field::Known(CurrentWindow {
            id: WindowId::parse("@1").unwrap(),
            width: Field::Known(88),
            height: Field::Known(26),
        }),
        active: Field::Known(false),
    };
    let snapshot =
        SessionSnapshot::new(vec![session], UNIX_EPOCH, InvokingContext::OutsideTmux).unwrap();
    let mut state = AppState::new(now);
    assert!(state.begin_due_request(now));
    state.finish_request(now, Ok(snapshot));
    state
}

fn render(state: &mut AppState, width: u16, height: u16, now: Instant) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| view::render(frame, state, now, false))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

#[test]
fn layout_switches_at_120_columns_and_supports_minimum_size() {
    let now = Instant::now();
    let mut state = build_state("project", now);
    for (width, height) in [(120, 30), (119, 30), (80, 24), (40, 10)] {
        let rows = render(&mut state, width, height, now);
        assert!(rows[2].contains("project"), "{width}: {rows:?}");
        assert!(rows[2].starts_with(">  "), "{width}: {:?}", rows[2]);
        if width >= 120 {
            assert!(rows[1].contains("WINDOWS") && rows[2].contains("88x26"));
        } else {
            assert!(!rows[1].contains("WINDOWS") && !rows[2].contains("88x26"));
        }
        assert!(rows[usize::from(height - 4)].contains("Name: project"));
    }
}

#[test]
fn names_are_truncated_at_grapheme_boundaries_and_forty_cells_fit() {
    let now = Instant::now();
    let exact = format!("{}界", "a".repeat(38));
    let mut state = build_state(&exact, now);
    let rows = render(&mut state, 120, 30, now);
    assert!(rows[2].contains(&exact), "{:?}", rows[2]);
    assert!(!rows[2].contains("..."));

    let long = format!("{}e\u{301}界-extra", "a".repeat(36));
    let mut state = build_state(&long, now);
    let rows = render(&mut state, 120, 30, now);
    assert!(rows[2].contains("e\u{301}..."), "{:?}", rows[2]);
    assert!(!rows[2].contains('\u{fffd}'));
}

#[test]
fn crawl_blanks_a_partially_visible_wide_character() {
    let now = Instant::now();
    let mut state = build_state("a界b", now);
    let rows = render(&mut state, 40, 10, now + Duration::from_secs(8));
    assert!(rows[6].starts_with(" b"), "{:?}", rows[6]);
}

#[test]
fn crawl_recovers_wide_characters_clipped_at_the_right_edge() {
    let now = Instant::now();
    let mut state = build_state(&format!("{}界tail", "a".repeat(33)), now);
    let initial = render(&mut state, 40, 10, now);
    assert!(!initial[6].contains('界'), "{:?}", initial[6]);
    let advanced = render(&mut state, 40, 10, now + Duration::from_secs(1));
    assert!(advanced[6].contains('界'), "{:?}", advanced[6]);
}

#[test]
fn control_characters_are_escaped_in_rows_and_crawl() {
    let now = Instant::now();
    let mut state = build_state("name\x1b\tend", now);
    let rows = render(&mut state, 80, 24, now);
    assert!(rows[2].contains("name\\u{1b}\\tend"), "{:?}", rows[2]);
    assert!(rows[20].contains("name\\u{1b}\\tend"), "{:?}", rows[20]);
    assert!(rows.iter().all(|line| !line.contains('\x1b')));
}

#[test]
fn undersized_and_zero_areas_are_safe_and_recover() {
    let now = Instant::now();
    let mut state = build_state("project", now);
    for (width, height) in [(39, 10), (40, 9)] {
        let rows = render(&mut state, width, height, now);
        assert!(rows[0].contains("Terminal too small"), "{rows:?}");
    }
    assert!(render(&mut state, 0, 0, now).is_empty());
    assert!(render(&mut state, 0, 10, now).iter().all(String::is_empty));
    assert!(render(&mut state, 40, 0, now).is_empty());
    let rows = render(&mut state, 40, 10, now);
    assert!(rows[2].contains("project"));
    assert_eq!(state.focus.as_ref().unwrap().as_str(), "$1");
}
