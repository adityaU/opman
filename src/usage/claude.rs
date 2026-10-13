//! Claude subscription usage, read from the endpoint Claude Code's own `/usage` uses.

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::Value;

use super::credentials::{self, ClaudeLogin, Login};
use super::types::{normalize_iso, Account, AccountId, AccountState, Credits, Severity, Window};

pub const DEFAULT_BASE: &str = "https://api.anthropic.com";
const ID: AccountId = AccountId::Claude;
/// How long the provider gets to answer, matching the CLIs' own budget.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Where to look for the login and whom to ask.
#[derive(Clone, Debug)]
pub struct ClaudeSource {
    pub base_url: String,
    pub timeout: Duration,
    pub config_dir: Option<std::path::PathBuf>,
    pub keychain: bool,
}

pub async fn account(client: &Client, source: &ClaudeSource) -> Account {
    let login = credentials::claude_login(source.config_dir.as_deref(), source.keychain).await;
    let login = match login {
        Login::Found(login) => login,
        Login::Missing => {
            return Account::new(ID, AccountState::SignedOut)
                .with_note("Sign in with `claude` to see usage")
        }
        Login::Unreadable(why) => return Account::error(ID, why),
    };
    let plan = login.plan.clone();
    if is_expired(&login, chrono::Utc::now().timestamp_millis()) {
        return Account::expired(ID).with_plan(plan);
    }
    fetch(client, source, &login).await.with_plan(plan)
}

pub fn is_expired(login: &ClaudeLogin, now_ms: i64) -> bool {
    login.expires_at_ms.is_some_and(|at| at <= now_ms)
}

async fn fetch(client: &Client, source: &ClaudeSource, login: &ClaudeLogin) -> Account {
    let url = format!("{}/api/oauth/usage", source.base_url.trim_end_matches('/'));
    let sent = client
        .get(url)
        .bearer_auth(login.access_token.expose())
        .header("anthropic-beta", "oauth-2025-04-20")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .timeout(source.timeout)
        .send()
        .await;
    let response = match sent {
        Ok(response) => response,
        Err(error) if error.is_timeout() => {
            return Account::error(ID, "Anthropic did not answer in time")
        }
        Err(_) => return Account::error(ID, "Could not reach Anthropic"),
    };
    match response.status() {
        StatusCode::UNAUTHORIZED => return Account::expired(ID),
        status if !status.is_success() => {
            return Account::error(ID, format!("Anthropic answered {}", status.as_u16()))
        }
        _ => {}
    }
    match response.json::<Value>().await {
        Ok(body) => parse(&body),
        Err(_) => Account::error(ID, "Anthropic sent an unreadable usage report"),
    }
}

/// The usage report. The generic `limits` list is preferred; the older fixed
/// `five_hour` / `seven_day` fields are the fallback.
pub fn parse(body: &Value) -> Account {
    let mut windows = from_limits(body);
    if windows.is_empty() {
        windows = from_legacy(body);
    }
    let mut account = Account::new(ID, AccountState::Ok);
    account.windows = windows;
    account.credits = credits(body);
    account
}

fn from_limits(body: &Value) -> Vec<Window> {
    let Some(limits) = body.get("limits").and_then(Value::as_array) else {
        return Vec::new();
    };
    limits.iter().filter_map(limit_window).collect()
}

fn limit_window(limit: &Value) -> Option<Window> {
    let kind = limit.get("kind")?.as_str()?;
    let percent = limit.get("percent")?.as_f64()?;
    let (id, label) = name_kind(kind);
    let mut window = Window::new(id, label, percent).resets_at(resets_at(limit));
    if let Some(severity) = limit
        .get("severity")
        .and_then(Value::as_str)
        .and_then(Severity::parse)
    {
        window.severity = Some(severity);
    }
    Some(window)
}

fn name_kind(kind: &str) -> (String, String) {
    match kind {
        "session" => ("session".into(), "5-hour".into()),
        "weekly_all" | "weekly" => ("weekly".into(), "Weekly".into()),
        other => match other.strip_prefix("weekly_") {
            Some(model) => (other.to_string(), format!("Weekly · {}", title(model))),
            None => (other.to_string(), title(other)),
        },
    }
}

fn title(raw: &str) -> String {
    raw.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn from_legacy(body: &Value) -> Vec<Window> {
    const FIELDS: [(&str, &str, &str); 4] = [
        ("five_hour", "session", "5-hour"),
        ("seven_day", "weekly", "Weekly"),
        ("seven_day_opus", "weekly_opus", "Weekly · Opus"),
        ("seven_day_sonnet", "weekly_sonnet", "Weekly · Sonnet"),
    ];
    FIELDS
        .iter()
        .filter_map(|(field, id, label)| {
            let entry = body.get(*field)?;
            let used = entry.get("utilization")?.as_f64()?;
            Some(Window::new(*id, *label, used).resets_at(resets_at(entry)))
        })
        .collect()
}

fn resets_at(entry: &Value) -> Option<String> {
    entry
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(normalize_iso)
}

/// Extra usage (pay-as-you-go credits past the plan). The balance is what remains of
/// the monthly limit, when the provider reports both numbers.
fn credits(body: &Value) -> Option<Credits> {
    let extra = body.get("extra_usage").filter(|extra| extra.is_object())?;
    let enabled = extra
        .get("is_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let limit = extra.get("monthly_limit").and_then(Value::as_f64);
    let used = extra.get("used_credits").and_then(Value::as_f64);
    let balance = match (limit, used) {
        (Some(limit), Some(used)) => Some(format_amount(limit - used)),
        _ => None,
    };
    Some(Credits { balance, enabled })
}

fn format_amount(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
