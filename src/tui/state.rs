use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::model::{
    Field, Session, SessionId, SessionSnapshot, elapsed_uptime, escape_text, format_uptime, uptime,
};

pub const HELP_LINES: &[&str] = &[
    "Sagebrush is a read-only tmux session browser.",
    "",
    "j / k    Move list focus or scroll help.",
    "?        Open this help.",
    "Esc      Return to the session list.",
    "q        Quit and restore the terminal.",
    "",
    "Focus selects a row for inspection; it does not change tmux.",
    "* marks the invoking session; ? means active status is unavailable.",
    "Sessions refresh automatically. Failed requests retry after five seconds.",
    "The selected session's complete details crawl across the screen.",
    "The crawl moves one cell per second and repeats automatically.",
    "A partial-data warning means one or more values are unavailable.",
    "For an error, check tmux and the selected server, then wait for retry.",
    "A terminal of at least 40 columns by 10 rows is recommended.",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    List,
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadState {
    Loading,
    Fresh,
    Partial,
    Refreshing,
    Failed(String),
    Stale(String),
}

struct CrawlPass {
    text: String,
    started: Instant,
    partial: bool,
}

pub struct AppState {
    pub screen: Screen,
    pub snapshot: Option<SessionSnapshot>,
    pub focus: Option<SessionId>,
    pub help_scroll: usize,
    pub list_scroll: usize,
    pub viewport: (u16, u16),
    pub load_state: LoadState,
    pub last_error: Option<String>,
    in_flight: bool,
    next_request: Instant,
    crawl: Option<CrawlPass>,
}

impl AppState {
    pub fn new(now: Instant) -> Self {
        Self {
            screen: Screen::List,
            snapshot: None,
            focus: None,
            help_scroll: 0,
            list_scroll: 0,
            viewport: (0, 0),
            load_state: LoadState::Loading,
            last_error: None,
            in_flight: false,
            next_request: now,
            crawl: None,
        }
    }

    pub fn begin_due_request(&mut self, now: Instant) -> bool {
        if self.in_flight || now < self.next_request {
            return false;
        }
        self.in_flight = true;
        self.next_request = now + Duration::from_secs(1);
        self.load_state = if self.snapshot.is_some() {
            LoadState::Refreshing
        } else {
            LoadState::Loading
        };
        true
    }

    pub fn finish_request(&mut self, now: Instant, result: Result<SessionSnapshot, String>) {
        self.in_flight = false;
        match result {
            Ok(snapshot) => {
                self.last_error = None;
                if self.next_request <= now {
                    let missed = now.duration_since(self.next_request).as_secs() + 1;
                    self.next_request += Duration::from_secs(missed);
                }
                let old_focus = self.focus.clone();
                self.focus = select_focus(old_focus.as_ref(), &snapshot.sessions);
                self.load_state = if snapshot.issues.is_empty() {
                    LoadState::Fresh
                } else {
                    LoadState::Partial
                };
                self.snapshot = Some(snapshot);
                if self.focus != old_focus || self.crawl.is_none() {
                    self.restart_crawl(now);
                }
                self.keep_focus_visible();
            }
            Err(error) => {
                self.next_request = now + Duration::from_secs(5);
                self.last_error = Some(error.clone());
                self.load_state = if self.snapshot.is_some() {
                    LoadState::Stale(error)
                } else {
                    LoadState::Failed(error)
                };
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyCode, now: Instant) -> bool {
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Char('?') => self.screen = Screen::Help,
            KeyCode::Esc => self.screen = Screen::List,
            KeyCode::Char('j') if self.screen == Screen::Help => {
                self.help_scroll = self
                    .help_scroll
                    .saturating_add(1)
                    .min(self.max_help_scroll());
            }
            KeyCode::Char('k') if self.screen == Screen::Help => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            KeyCode::Char('j') | KeyCode::Char('k') if self.screen == Screen::List => {
                if let (Some(snapshot), Some(focus)) = (&self.snapshot, &self.focus)
                    && let Some(index) = snapshot.sessions.iter().position(|row| row.id == *focus)
                {
                    let next = if key == KeyCode::Char('j') {
                        (index + 1).min(snapshot.sessions.len() - 1)
                    } else {
                        index.saturating_sub(1)
                    };
                    let next_focus = snapshot.sessions[next].id.clone();
                    if next_focus != *focus {
                        self.focus = Some(next_focus);
                        self.restart_crawl(now);
                        self.keep_focus_visible();
                    }
                }
            }
            _ => {}
        }
        false
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = (width, height);
        self.help_scroll = self.help_scroll.min(self.max_help_scroll());
        self.keep_focus_visible();
    }

    pub fn partial_warning(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.issues.is_empty())
            || self.crawl.as_ref().is_some_and(|crawl| crawl.partial)
    }

    pub fn crawl_text(&self) -> Option<&str> {
        self.crawl.as_ref().map(|crawl| crawl.text.as_str())
    }

    pub fn crawl_offset(&self, now: Instant) -> usize {
        self.crawl
            .as_ref()
            .map(|crawl| now.saturating_duration_since(crawl.started).as_secs() as usize)
            .unwrap_or(0)
    }

    pub fn advance_crawl(&mut self, now: Instant) {
        let finished = self.crawl.as_ref().is_some_and(|crawl| {
            let cells = UnicodeWidthStr::width(crawl.text.as_str());
            let length = cells.saturating_add(usize::from(self.viewport.0.max(1)));
            now.saturating_duration_since(crawl.started).as_secs() >= length as u64
        });
        if finished {
            self.restart_crawl(now);
        }
    }

    fn restart_crawl(&mut self, now: Instant) {
        self.crawl = self.snapshot.as_ref().and_then(|snapshot| {
            let focus = self.focus.as_ref()?;
            let session = snapshot.sessions.iter().find(|row| &row.id == focus)?;
            Some(CrawlPass {
                text: details(session, snapshot, now),
                started: now,
                partial: !snapshot.issues.is_empty(),
            })
        });
    }

    fn keep_focus_visible(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            self.list_scroll = 0;
            return;
        };
        let visible = usize::from(self.viewport.1.saturating_sub(7).max(1));
        let Some(index) = self.focus.as_ref().and_then(|focus| {
            snapshot
                .sessions
                .iter()
                .position(|session| &session.id == focus)
        }) else {
            self.list_scroll = 0;
            return;
        };
        self.list_scroll = self
            .list_scroll
            .min(snapshot.sessions.len().saturating_sub(visible));
        if index < self.list_scroll {
            self.list_scroll = index;
        } else if index >= self.list_scroll.saturating_add(visible) {
            self.list_scroll = index + 1 - visible;
        }
    }

    fn max_help_scroll(&self) -> usize {
        let visible = usize::from(self.viewport.1.saturating_sub(2).max(1));
        wrapped_help(self.viewport.0).len().saturating_sub(visible)
    }
}

pub fn wrapped_help(width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    for text in HELP_LINES {
        let mut line = String::new();
        let mut cells = 0;
        for grapheme in text.graphemes(true) {
            let size = UnicodeWidthStr::width(grapheme);
            if cells > 0 && cells + size > width {
                lines.push(std::mem::take(&mut line));
                cells = 0;
            }
            line.push_str(grapheme);
            cells += size;
        }
        lines.push(line);
    }
    lines
}

fn select_focus(previous: Option<&SessionId>, sessions: &[Session]) -> Option<SessionId> {
    if sessions.is_empty() {
        return None;
    }
    match previous {
        Some(previous) => sessions
            .iter()
            .find(|session| session.id >= *previous)
            .or_else(|| sessions.last())
            .map(|session| session.id.clone()),
        None => Some(sessions[0].id.clone()),
    }
}

fn details(session: &Session, snapshot: &SessionSnapshot, now: Instant) -> String {
    let name = match &session.name {
        Field::Known(name) => escape_text(name),
        Field::Unavailable(_) => "-".to_owned(),
    };
    let windows = field_number(&session.window_count);
    let uptime = match uptime(&session.created_at, snapshot.captured_at) {
        Field::Known(seconds) => elapsed_uptime(
            seconds,
            now.saturating_duration_since(snapshot.captured_monotonic),
        )
        .map(format_uptime)
        .unwrap_or_else(|| "-".to_owned()),
        Field::Unavailable(_) => "-".to_owned(),
    };
    let size = match &session.current_window {
        Field::Known(window) => format!(
            "{}x{}",
            field_number(&window.width),
            field_number(&window.height)
        ),
        Field::Unavailable(_) => "-x-".to_owned(),
    };
    let active = match session.active {
        Field::Known(true) => "yes",
        Field::Known(false) => "no",
        Field::Unavailable(_) => "-",
    };
    let mut details = format!(
        "Name: {name}  Windows: {windows}  Uptime: {uptime}  Size: {size}  Active: {active}"
    );
    for issue in snapshot
        .issues
        .iter()
        .filter(|issue| issue.session_id.as_ref().is_none_or(|id| id == &session.id))
    {
        details.push_str(&format!(
            "  Issue: {} {}",
            escape_text(&issue.field),
            escape_text(&issue.message)
        ));
        if let Some(recovery) = &issue.recovery {
            details.push_str(&format!(" {}", escape_text(recovery)));
        }
    }
    details
}

fn field_number(field: &Field<u32>) -> String {
    match field {
        Field::Known(value) => value.to_string(),
        Field::Unavailable(_) => "-".to_owned(),
    }
}
