//! Recognising "this account has run out" in the error text an agent reports.
//!
//! This is the one place opman reads limit prose. Every engine that surfaces a turn
//! failure tags its error with [`tag`], so the web UI gets a structured `limit` field and
//! never has to pattern-match provider wording itself — wording that differs between
//! Claude, Codex and the generic HTTP 429, and that changes from release to release.

use chrono::{SecondsFormat, TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;

/// Why the turn was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    /// The plan's allowance is spent until its window resets.
    Usage,
    /// Throttled: too many requests right now, usually clears on its own.
    Rate,
}

/// A recognised limit failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitHit {
    pub kind: LimitKind,
    /// When the provider named the reset moment in a machine-readable form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
}

/// Phrases that mean the plan's allowance is spent. Matched against lowercased text with
/// curly apostrophes folded to straight ones.
const USAGE_PHRASES: &[&str] = &[
    "usage limit reached",
    "usage_limit_reached",
    "usage_limit_exceeded",
    "usage limit exceeded",
    "you've hit your usage limit",
];

/// Phrases that mean the request was throttled.
const RATE_PHRASES: &[&str] = &[
    "rate limit",
    "rate_limit",
    "rate-limit",
    "ratelimit",
    "too many requests",
];

/// Classify an error message. `None` for anything that is not a limit.
pub fn classify(text: &str) -> Option<LimitHit> {
    let folded = text.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'");
    let usage = USAGE_PHRASES.iter().any(|p| folded.contains(p)) || hit_your_limit(&folded);
    let kind = if usage {
        LimitKind::Usage
    } else if RATE_PHRASES.iter().any(|p| folded.contains(p)) {
        LimitKind::Rate
    } else {
        return None;
    };
    Some(LimitHit {
        kind,
        resets_at: epoch_reset(text),
    })
}

/// The value to put on a message's `limit` field, or `None` when the text is no limit.
pub fn tag(text: &str) -> Option<Value> {
    classify(text).and_then(|hit| serde_json::to_value(hit).ok())
}

/// "You've hit your limit", "You've hit your 5-hour limit", "You have hit your weekly
/// limit": the claim and the word "limit" in the same sentence.
fn hit_your_limit(folded: &str) -> bool {
    ["you've hit your", "you have hit your", "you've reached your", "you have reached your"]
        .iter()
        .filter_map(|lead| folded.find(lead).map(|at| &folded[at + lead.len()..]))
        .any(|rest| {
            let sentence = rest.split(['.', '\n']).next().unwrap_or(rest);
            sentence.contains("limit")
        })
}

/// Claude's CLI writes `Claude AI usage limit reached|1760000000` — the reset as a Unix
/// timestamp after a pipe. Anything else that looks like a time is prose ("resets 3pm")
/// and is left to the usage report, which knows the real window.
fn epoch_reset(text: &str) -> Option<String> {
    let (_, tail) = text.split_once('|')?;
    let digits: String = tail.trim().chars().take_while(char::is_ascii_digit).collect();
    let mut secs: i64 = digits.parse().ok()?;
    // Milliseconds sometimes; seconds are 10 digits until 2286.
    if digits.len() > 11 {
        secs /= 1000;
    }
    if secs < 1_000_000_000 {
        return None;
    }
    Utc.timestamp_opt(secs, 0)
        .single()
        .map(|at| at.to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[cfg(test)]
#[path = "limit_error_tests.rs"]
mod tests;
