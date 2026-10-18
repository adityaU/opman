//! Antigravity usage, read from the agent itself: its ACP server answers the `/usage`
//! command with the same quota lines its own CLI shows. There is no HTTP call and no token
//! handling here; the agent's own login does the work.

use std::time::Duration;

use chrono::{NaiveDateTime, TimeZone, Utc};
use serde_json::Value;

use crate::acp_engine::config::{self, AgentConfig};
use crate::acp_engine::oneshot::{self, Answer, Failure};

use super::types::{Account, AccountId, AccountState, Window};

const ID: AccountId = AccountId::Antigravity;
/// The agent runs in the `antigravity` runner slot.
pub const RUNNER: &str = "antigravity";
/// Starting the agent takes a few seconds, and the quota call is a network round trip.
pub const TIMEOUT: Duration = Duration::from_secs(25);

/// Which agent to ask.
#[derive(Clone, Debug, Default)]
pub enum Agent {
    /// Whatever the user has enabled in the `antigravity` slot.
    #[default]
    FromConfig,
    /// This one, whether or not it is enabled. Tests use it.
    Use(Box<AgentConfig>),
    /// None: no row is shown.
    Off,
}

#[derive(Clone, Debug)]
pub struct AntigravitySource {
    pub agent: Agent,
    pub timeout: Duration,
}

impl Default for AntigravitySource {
    fn default() -> Self {
        Self {
            agent: Agent::default(),
            timeout: TIMEOUT,
        }
    }
}

/// `None` when Antigravity is not enabled, so no row is shown for it.
pub async fn account(source: &AntigravitySource) -> Option<Account> {
    let agent = match &source.agent {
        Agent::Use(agent) => (**agent).clone(),
        Agent::FromConfig => config::load().for_runner(RUNNER)?.1.clone(),
        Agent::Off => return None,
    };
    // A bare command name is found on the PATH by the spawn; a path must exist already.
    if agent.command.contains('/') && !std::path::Path::new(&agent.command).is_file() {
        return Some(
            Account::new(ID, AccountState::SignedOut)
                .with_note("Antigravity is not downloaded yet; start a session with it first"),
        );
    }
        Some(match oneshot::ask(&agent, "/usage", source.timeout).await {
        Ok(answer) => from_answer(&answer),
        Err(Failure::LoginRequired) => Account::new(ID, AccountState::SignedOut)
            .with_note(ID.relogin_note()),
        Err(Failure::Other(reason)) => Account::error(ID, first_line(&reason)),
    })
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).to_string()
}

fn from_answer(answer: &Answer) -> Account {
    let windows = parse_windows(&answer.text);
    let plan = answer
        .session
        .pointer("/_meta/antigravity/tier/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    if windows.is_empty() {
        return Account::error(ID, "Antigravity did not report any quota limits").with_plan(plan);
    }
    let mut account = Account::new(ID, AccountState::Ok).with_plan(plan);
    account.windows = windows;
    account
}

/// Read the quota lines out of the `/usage` text. Groups are `- **Name**` lines; limits are
/// `- **<what> Limit Remaining:** 87% remaining (resets Oct 25, 2026, 12:17 UTC)` under them.
pub fn parse_windows(text: &str) -> Vec<Window> {
    let mut group = String::new();
    let mut windows = Vec::new();
    for line in text.lines() {
        let Some(item) = line.trim().strip_prefix("- **") else {
            continue;
        };
        let Some((name, rest)) = item.split_once("**") else {
            continue;
        };
        let Some(label) = name.strip_suffix(':') else {
            group = short_group(name);
            continue;
        };
        if let Some(window) = window(&group, label, rest) {
            windows.push(window);
        }
    }
    windows
}

fn window(group: &str, label: &str, rest: &str) -> Option<Window> {
    let (percent, reset) = remaining(rest)?;
    let label = window_label(group, label);
    let id = label.to_lowercase().replace(' ', "-");
    Some(Window::new(id, label, 100.0 - percent).resets_at(reset))
}

/// "100% remaining (resets Oct 18, 2026, 17:17 UTC)" -> (100.0, Some(ISO time)).
fn remaining(rest: &str) -> Option<(f64, Option<String>)> {
    let (number, tail) = rest.trim().split_once('%')?;
    let percent: f64 = number.trim().parse().ok()?;
    let reset = tail
        .split_once("(resets ")
        .and_then(|(_, at)| at.split_once(')'))
        .and_then(|(at, _)| reset_time(at));
    Some((percent.clamp(0.0, 100.0), reset))
}

fn reset_time(text: &str) -> Option<String> {
    let text = text.trim().strip_suffix("UTC")?.trim();
    let naive = NaiveDateTime::parse_from_str(text, "%b %d, %Y, %H:%M").ok()?;
    Some(super::types::iso_from(Utc.from_utc_datetime(&naive)))
}

/// "Gemini Models" -> "Gemini", "Claude and GPT models" -> "Claude and GPT".
fn short_group(name: &str) -> String {
    let name = name.trim();
    let lower = name.to_lowercase();
    match lower.strip_suffix(" models") {
        Some(_) => name[..name.len() - " models".len()].to_string(),
        None => name.to_string(),
    }
}

/// "Weekly Limit Remaining" in group "Gemini" -> "Gemini weekly"; a label that already
/// names its group, such as "Gemini 4 Argon Weekly Limit", is only cleaned.
fn window_label(group: &str, label: &str) -> String {
    let cleaned = label
        .trim_end_matches(" Remaining")
        .trim_end_matches(" Limit")
        .replace("Five Hour", "5-hour");
    if group.is_empty() || cleaned.starts_with(group) {
        return cleaned;
    }
    format!("{group} {}", cleaned.to_lowercase())
}

#[cfg(test)]
#[path = "antigravity_tests.rs"]
mod antigravity_tests;
