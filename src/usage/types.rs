//! The `/api/usage` wire contract. Everything here is what leaves the process, so no
//! type in this file may ever hold a credential.

use std::collections::BTreeMap;

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde::Serialize;

/// A provider account opman can read usage for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountId {
    Claude,
    Codex,
}

impl AccountId {
    pub fn label(self) -> &'static str {
        match self {
            AccountId::Claude => "Claude",
            AccountId::Codex => "Codex",
        }
    }

    /// What to tell the user when the login has lapsed. opman never refreshes a token
    /// itself: refresh tokens are single-use, so doing so would sign the CLI out.
    pub fn relogin_note(self) -> &'static str {
        match self {
            AccountId::Claude => "Open `claude` once to refresh its login",
            AccountId::Codex => "Open `codex` once to refresh its login",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    Ok,
    Expired,
    SignedOut,
    ApiKey,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Normal,
    Warning,
    Critical,
}

impl Severity {
    /// The provider's own word when it gives one we know.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "normal" => Some(Severity::Normal),
            "warning" => Some(Severity::Warning),
            "critical" | "error" | "exceeded" => Some(Severity::Critical),
            _ => None,
        }
    }

    /// Derived from the percentage when the provider gives none.
    pub fn from_percent(used: f64) -> Self {
        if used >= 90.0 {
            Severity::Critical
        } else if used >= 75.0 {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub id: String,
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<String>,
    pub severity: Option<Severity>,
}

impl Window {
    pub fn new(id: impl Into<String>, label: impl Into<String>, used_percent: f64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            used_percent,
            resets_at: None,
            severity: Some(Severity::from_percent(used_percent)),
        }
    }

    pub fn resets_at(mut self, at: Option<String>) -> Self {
        self.resets_at = at;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    pub balance: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: AccountId,
    pub label: &'static str,
    pub state: AccountState,
    pub plan: Option<String>,
    pub windows: Vec<Window>,
    pub credits: Option<Credits>,
    pub note: Option<String>,
    pub stale: bool,
    pub fetched_at: String,
}

impl Account {
    /// An account with no data yet, stamped now.
    pub fn new(id: AccountId, state: AccountState) -> Self {
        Self {
            id,
            label: id.label(),
            state,
            plan: None,
            windows: Vec::new(),
            credits: None,
            note: None,
            stale: false,
            fetched_at: now_iso(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_plan(mut self, plan: Option<String>) -> Self {
        self.plan = plan;
        self
    }

    pub fn expired(id: AccountId) -> Self {
        Self::new(id, AccountState::Expired).with_note(id.relogin_note())
    }

    pub fn error(id: AccountId, note: impl Into<String>) -> Self {
        Self::new(id, AccountState::Error).with_note(note)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    pub accounts: Vec<Account>,
    pub runners: BTreeMap<String, AccountId>,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// A provider timestamp re-emitted in one canonical form; unparseable input is dropped
/// rather than passed through, so the frontend only ever sees valid ISO-8601.
pub fn normalize_iso(value: &str) -> Option<String> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

pub fn unix_to_iso(seconds: i64) -> Option<String> {
    let at = Utc.timestamp_opt(seconds, 0).single()?;
    Some(at.to_rfc3339_opts(SecondsFormat::Secs, true))
}

/// "5-hour", "Weekly", "30-day": a reset window named the way the vendors' CLIs do.
pub fn window_from_minutes(minutes: i64) -> (String, String) {
    match minutes {
        300 => ("session".into(), "5-hour".into()),
        10080 => ("weekly".into(), "Weekly".into()),
        m if m > 0 && m % 1440 == 0 => (format!("{}d", m / 1440), format!("{}-day", m / 1440)),
        m if m > 0 && m % 60 == 0 => (format!("{}h", m / 60), format!("{}-hour", m / 60)),
        m => (format!("{m}m"), format!("{m}-minute")),
    }
}
