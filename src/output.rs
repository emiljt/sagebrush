use serde::Serialize;

use crate::model::{Field, Session, SessionSnapshot, escape_text, format_uptime, uptime};

pub fn table(snapshot: &SessionSnapshot, terminal: bool) -> String {
    if snapshot.sessions.is_empty() {
        return if terminal {
            "No tmux sessions found.\n".to_owned()
        } else {
            String::new()
        };
    }
    let mut output = String::from("NAME\tWINDOWS\tUPTIME\tSIZE\tACTIVE\n");
    for session in &snapshot.sessions {
        output.push_str(&human_record(session, snapshot));
    }
    output
}

pub fn plain(snapshot: &SessionSnapshot) -> String {
    let mut output = String::new();
    for session in &snapshot.sessions {
        output.push_str(&human_record(session, snapshot));
    }
    output
}

#[derive(Serialize)]
struct JsonSession<'a> {
    name: Option<&'a str>,
    windows: Option<u32>,
    uptime: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    active: Option<bool>,
}

pub fn json(snapshot: &SessionSnapshot) -> serde_json::Result<String> {
    let rows: Vec<_> = snapshot
        .sessions
        .iter()
        .map(|session| {
            let (width, height) = match &session.current_window {
                Field::Known(window) => (known(&window.width), known(&window.height)),
                Field::Unavailable(_) => (None, None),
            };
            JsonSession {
                name: match &session.name {
                    Field::Known(name) => Some(name.as_str()),
                    Field::Unavailable(_) => None,
                },
                windows: known(&session.window_count),
                uptime: known(&uptime(&session.created_at, snapshot.captured_at)),
                width,
                height,
                active: known(&session.active),
            }
        })
        .collect();
    let mut output = serde_json::to_string_pretty(&rows)?;
    output.push('\n');
    Ok(output)
}

fn known<T: Copy>(field: &Field<T>) -> Option<T> {
    match field {
        Field::Known(value) => Some(*value),
        Field::Unavailable(_) => None,
    }
}

fn human_record(session: &Session, snapshot: &SessionSnapshot) -> String {
    let name = match &session.name {
        Field::Known(name) => escape_text(name),
        Field::Unavailable(_) => "unavailable".to_owned(),
    };
    let windows = number(&session.window_count);
    let uptime = match uptime(&session.created_at, snapshot.captured_at) {
        Field::Known(seconds) => format_uptime(seconds),
        Field::Unavailable(_) => "unavailable".to_owned(),
    };
    let size = match &session.current_window {
        Field::Known(window) => format!("{}x{}", number(&window.width), number(&window.height)),
        Field::Unavailable(_) => "unavailablexunavailable".to_owned(),
    };
    let active = match &session.active {
        Field::Known(true) => "yes",
        Field::Known(false) => "no",
        Field::Unavailable(_) => "unavailable",
    };
    format!("{name}\t{windows}\t{uptime}\t{size}\t{active}\n")
}

pub fn diagnostics(snapshot: &SessionSnapshot) -> String {
    let mut output = String::new();
    for issue in &snapshot.issues {
        let id = issue
            .session_id
            .as_ref()
            .map(|id| id.as_str())
            .unwrap_or("session");
        output.push_str(&format!(
            "{}: {}: {}",
            escape_text(id),
            escape_text(&issue.field),
            escape_text(&issue.message)
        ));
        if let Some(recovery) = &issue.recovery {
            output.push_str(&format!("; {}", escape_text(recovery)));
        }
        output.push('\n');
    }
    output
}

fn number(field: &Field<u32>) -> String {
    match field {
        Field::Known(value) => value.to_string(),
        Field::Unavailable(_) => "unavailable".to_owned(),
    }
}
