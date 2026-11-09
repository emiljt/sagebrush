use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let digits = value
            .strip_prefix('$')
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| format!("invalid tmux session identity: {}", escape_text(value)))?;
        let canonical = digits.trim_start_matches('0');
        Ok(Self(format!(
            "${}",
            if canonical.is_empty() { "0" } else { canonical }
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Ord for SessionId {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = &self.0[1..];
        let right = &other.0[1..];
        left.len().cmp(&right.len()).then_with(|| left.cmp(right))
    }
}

impl PartialOrd for SessionId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WindowId(String);

impl WindowId {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value
            .strip_prefix('@')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(format!(
                "invalid tmux window identity: {}",
                escape_text(value)
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueCategory {
    MissingTmux,
    UnsupportedVersion,
    Connection,
    MalformedOutput,
    InvalidText,
    Context,
    Timeout,
    Cancellation,
    ResourceExhaustion,
    Clock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldIssue {
    pub session_id: Option<SessionId>,
    pub field: String,
    pub category: IssueCategory,
    pub message: String,
    pub recovery: Option<String>,
}

impl From<&str> for FieldIssue {
    fn from(message: &str) -> Self {
        Self {
            session_id: None,
            field: String::new(),
            category: IssueCategory::MalformedOutput,
            message: message.to_owned(),
            recovery: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Field<T> {
    Known(T),
    Unavailable(FieldIssue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentWindow {
    pub id: WindowId,
    pub width: Field<u32>,
    pub height: Field<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub name: Field<String>,
    pub window_count: Field<u32>,
    pub created_at: Field<SystemTime>,
    pub current_window: Field<CurrentWindow>,
    pub active: Field<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvokingContext {
    OutsideTmux,
    InsideTmux { session_id: SessionId },
    ActiveUnavailable,
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub sessions: Vec<Session>,
    pub captured_at: SystemTime,
    pub captured_monotonic: Instant,
    pub issues: Vec<FieldIssue>,
    pub context: InvokingContext,
}

impl SessionSnapshot {
    pub fn new(
        mut sessions: Vec<Session>,
        captured_at: SystemTime,
        mut context: InvokingContext,
    ) -> Result<Self, String> {
        if let InvokingContext::InsideTmux { session_id } = &context
            && !sessions.iter().any(|session| &session.id == session_id)
        {
            context = InvokingContext::ActiveUnavailable;
        }
        let mut ids = HashSet::new();
        for session in &mut sessions {
            if !ids.insert(session.id.clone()) {
                return Err(format!(
                    "duplicate tmux session identity: {}",
                    session.id.as_str()
                ));
            }
            validate_positive(&mut session.window_count, "windows");
            if let Field::Known(window) = &mut session.current_window {
                validate_positive(&mut window.width, "width");
                validate_positive(&mut window.height, "height");
            }
            if let Field::Known(created_at) = session.created_at
                && created_at.duration_since(SystemTime::UNIX_EPOCH).is_err()
            {
                session.created_at = Field::Unavailable(issue(
                    "created_at",
                    IssueCategory::MalformedOutput,
                    "session creation time must be nonnegative",
                ));
            }
            session.active = match &context {
                InvokingContext::OutsideTmux => Field::Known(false),
                InvokingContext::InsideTmux { session_id } => {
                    Field::Known(&session.id == session_id)
                }
                InvokingContext::ActiveUnavailable => Field::Unavailable(issue(
                    "active",
                    IssueCategory::Context,
                    "cannot reliably identify the invoking tmux client",
                )),
            };
        }
        sessions.sort_by(|left, right| left.id.cmp(&right.id));
        let mut issues = Vec::new();
        for session in &sessions {
            collect_issue(&mut issues, &session.id, &session.name);
            collect_issue(&mut issues, &session.id, &session.window_count);
            collect_issue(&mut issues, &session.id, &session.created_at);
            collect_issue(&mut issues, &session.id, &session.active);
            match &session.current_window {
                Field::Known(window) => {
                    collect_issue(&mut issues, &session.id, &window.width);
                    collect_issue(&mut issues, &session.id, &window.height);
                }
                Field::Unavailable(_) => {
                    collect_issue(&mut issues, &session.id, &session.current_window)
                }
            }
            if matches!(session.created_at, Field::Known(_)) {
                collect_issue(
                    &mut issues,
                    &session.id,
                    &uptime(&session.created_at, captured_at),
                );
            }
        }
        Ok(Self {
            sessions,
            captured_at,
            captured_monotonic: Instant::now(),
            issues,
            context,
        })
    }
}

fn collect_issue<T>(issues: &mut Vec<FieldIssue>, id: &SessionId, field: &Field<T>) {
    if let Field::Unavailable(issue) = field {
        let mut issue = issue.clone();
        if issue.session_id.is_none() {
            issue.session_id = Some(id.clone());
        }
        issues.push(issue);
    }
}

fn issue(field: &str, category: IssueCategory, message: impl Into<String>) -> FieldIssue {
    FieldIssue {
        session_id: None,
        field: field.to_owned(),
        category,
        message: message.into(),
        recovery: None,
    }
}

pub fn positive_field(value: &str, field: &str) -> Field<u32> {
    match value
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| value.parse::<u32>())
    {
        Some(Ok(number)) if number > 0 => Field::Known(number),
        _ => Field::Unavailable(issue(
            field,
            IssueCategory::MalformedOutput,
            format!("invalid {field}: expected a positive integer"),
        )),
    }
}

fn validate_positive(value: &mut Field<u32>, field: &str) {
    if matches!(value, Field::Known(0)) {
        *value = Field::Unavailable(issue(
            field,
            IssueCategory::MalformedOutput,
            format!("invalid {field}: expected a positive integer"),
        ));
    }
}

pub fn decode_name(value: &[u8], field: &str) -> Field<String> {
    match std::str::from_utf8(value) {
        Ok(name) => Field::Known(name.to_owned()),
        Err(_) => Field::Unavailable(issue(
            field,
            IssueCategory::InvalidText,
            "session name is not valid UTF-8",
        )),
    }
}

pub fn uptime(created_at: &Field<SystemTime>, captured_at: SystemTime) -> Field<u64> {
    match created_at {
        Field::Known(created_at) => match captured_at.duration_since(*created_at) {
            Ok(elapsed) => Field::Known(elapsed.as_secs()),
            Err(_) => {
                let mut problem = issue(
                    "uptime",
                    IssueCategory::Clock,
                    "session creation time is later than the current clock",
                );
                problem.recovery = Some("Check the system clock and retry.".to_owned());
                Field::Unavailable(problem)
            }
        },
        Field::Unavailable(issue) => Field::Unavailable(issue.clone()),
    }
}

pub fn elapsed_uptime(seconds: u64, elapsed: Duration) -> Option<u64> {
    seconds.checked_add(elapsed.as_secs())
}

pub fn format_uptime(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_owned();
    }
    let units = [
        (seconds / 86_400, 'd'),
        ((seconds % 86_400) / 3_600, 'h'),
        ((seconds % 3_600) / 60, 'm'),
        (seconds % 60, 's'),
    ];
    let first = units.iter().position(|(value, _)| *value != 0).unwrap();
    let last = units.iter().rposition(|(value, _)| *value != 0).unwrap();
    units[first..=last]
        .iter()
        .map(|(value, unit)| format!("{value}{unit}"))
        .collect()
}

pub fn escape_text(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            control if control.is_control() => escaped.extend(control.escape_unicode()),
            printable => escaped.push(printable),
        }
    }
    escaped
}
