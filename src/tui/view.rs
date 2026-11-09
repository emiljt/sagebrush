use std::ffi::OsStr;
use std::time::Instant;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::model::{
    Field, Session, SessionSnapshot, elapsed_uptime, escape_text, format_uptime, uptime,
};

use super::state::{AppState, LoadState, Screen, wrapped_help};

pub fn color_enabled(no_color: bool, no_color_env: Option<&OsStr>, term: Option<&OsStr>) -> bool {
    !no_color
        && !no_color_env.is_some_and(|value| !value.is_empty())
        && term != Some(OsStr::new("dumb"))
}

pub fn render(frame: &mut Frame<'_>, state: &mut AppState, now: Instant, color: bool) {
    let area = frame.area();
    state.resize(area.width, area.height);
    if area.width == 0 || area.height == 0 {
        return;
    }
    if area.width < 40 || area.height < 10 {
        line(
            frame,
            area,
            0,
            "Terminal too small: need 40x10. Resize or press q.",
            Style::default(),
        );
        return;
    }
    if state.screen == Screen::Help {
        line(
            frame,
            area,
            0,
            "Help - Esc returns, q quits",
            Style::default(),
        );
        for (index, text) in wrapped_help(area.width)
            .iter()
            .skip(state.help_scroll)
            .take(usize::from(area.height.saturating_sub(2)))
            .enumerate()
        {
            let y = index + 1;
            if y >= usize::from(area.height) {
                break;
            }
            line(frame, area, y as u16, text, Style::default());
        }
        line(
            frame,
            area,
            area.height - 1,
            "j/k: scroll  Esc: back  q: quit",
            Style::default(),
        );
        return;
    }

    state.advance_crawl(now);
    let status = match &state.load_state {
        LoadState::Loading => "Loading sessions...",
        LoadState::Fresh => "Sessions",
        LoadState::Partial => "Sessions - partial",
        LoadState::Refreshing => "Refreshing sessions...",
        LoadState::Failed(_) => "Failed to load sessions",
        LoadState::Stale(_) => "Sessions - stale",
    };
    let warning = if state.partial_warning() && !matches!(state.load_state, LoadState::Partial) {
        " - partial"
    } else {
        ""
    };
    line(
        frame,
        area,
        0,
        &format!("Sagebrush - {status}{warning}"),
        Style::default(),
    );
    if area.width >= 120 {
        line(
            frame,
            area,
            1,
            "  ACTIVE NAME                                     WINDOWS       UPTIME        SIZE",
            Style::default(),
        );
    } else {
        line(frame, area, 1, "  ACTIVE NAME", Style::default());
    }
    if let Some(snapshot) = &state.snapshot {
        if snapshot.sessions.is_empty() {
            line(frame, area, 2, "No tmux sessions found.", Style::default());
        } else {
            let rows = usize::from(area.height.saturating_sub(7));
            for (index, session) in snapshot
                .sessions
                .iter()
                .skip(state.list_scroll)
                .take(rows)
                .enumerate()
            {
                let focus = state.focus.as_ref() == Some(&session.id);
                let rendered = row(session, snapshot, focus, area.width, now);
                let style = if color && focus {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                line(frame, area, 2 + index as u16, &rendered, style);
            }
        }
    }
    if let Some(crawl) = state.crawl_text() {
        let visible = clip_cells(crawl, state.crawl_offset(now), usize::from(area.width));
        line(frame, area, area.height - 4, &visible, Style::default());
    }
    let diagnostic = if let Some(error) = &state.last_error {
        let label = if state.snapshot.is_some() {
            "stale"
        } else {
            "Error"
        };
        Some(format!(
            "{label}: {}. Check tmux; retrying automatically.",
            escape_text(error)
        ))
    } else if state.partial_warning() {
        let issue = state
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.issues.first());
        Some(match issue {
            Some(issue) => format!(
                "partial: {} - {}. {}",
                escape_text(&issue.field),
                escape_text(&issue.message),
                issue
                    .recovery
                    .as_deref()
                    .map(escape_text)
                    .unwrap_or_else(|| "Check tmux and retry.".into())
            ),
            None => {
                "partial: previous details remain visible until the crawl completes.".to_owned()
            }
        })
    } else {
        None
    };
    if let Some(diagnostic) = diagnostic {
        line(frame, area, area.height - 3, &diagnostic, Style::default());
    }
    line(
        frame,
        area,
        area.height - 2,
        "j/k: focus  ?: help  q: quit  *: active  ?: unavailable",
        Style::default(),
    );
}

fn line(frame: &mut Frame<'_>, area: Rect, y: u16, text: &str, style: Style) {
    if y < area.height {
        frame.render_widget(
            Paragraph::new(text.to_owned()).style(style),
            Rect::new(area.x, area.y + y, area.width, 1),
        );
    }
}

fn row(
    session: &Session,
    snapshot: &SessionSnapshot,
    focus: bool,
    width: u16,
    now: Instant,
) -> String {
    let marker = if focus { '>' } else { ' ' };
    let active = match session.active {
        Field::Known(true) => '*',
        Field::Known(false) => ' ',
        Field::Unavailable(_) => '?',
    };
    let name = match &session.name {
        Field::Known(name) => escape_text(name),
        Field::Unavailable(_) => "-".into(),
    };
    if width < 120 {
        return format!(
            "{marker} {active} {}",
            truncate_cells(&name, usize::from(width).saturating_sub(4))
        );
    }
    let name = truncate_cells(&name, 40);
    let padding = 40_usize.saturating_sub(UnicodeWidthStr::width(name.as_str()));
    let windows = field_number(&session.window_count);
    let elapsed = now.saturating_duration_since(snapshot.captured_monotonic);
    let uptime = match uptime(&session.created_at, snapshot.captured_at) {
        Field::Known(seconds) => elapsed_uptime(seconds, elapsed)
            .map(format_uptime)
            .unwrap_or_else(|| "-".into()),
        Field::Unavailable(_) => "-".into(),
    };
    let size = match &session.current_window {
        Field::Known(window) => format!(
            "{}x{}",
            field_number(&window.width),
            field_number(&window.height)
        ),
        Field::Unavailable(_) => "-x-".into(),
    };
    format!(
        "{marker} {active} {name}{} {windows:>7} {uptime:>12} {size:>11}",
        " ".repeat(padding)
    )
}

fn field_number(field: &Field<u32>) -> String {
    match field {
        Field::Known(value) => value.to_string(),
        Field::Unavailable(_) => "-".into(),
    }
}

fn truncate_cells(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    let suffix = ".".repeat(width.min(3));
    let available = width.saturating_sub(suffix.len());
    let mut result = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let cells = UnicodeWidthStr::width(grapheme);
        if used + cells > available {
            break;
        }
        result.push_str(grapheme);
        used += cells;
    }
    result.push_str(&suffix);
    result
}

fn clip_cells(text: &str, start: usize, width: usize) -> String {
    let end = start.saturating_add(width);
    let mut position = 0_usize;
    let mut result = String::new();
    for grapheme in text.graphemes(true) {
        let cells = UnicodeWidthStr::width(grapheme);
        let next = position.saturating_add(cells);
        if position >= end {
            break;
        }
        if next > start {
            if position >= start && next <= end {
                result.push_str(grapheme);
            } else {
                result.push_str(&" ".repeat(next.min(end).saturating_sub(position.max(start))));
            }
        }
        position = next;
    }
    result
}
