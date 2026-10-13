//! Codex (ChatGPT login) usage, read from the endpoint the Codex CLI's `/status` uses,
//! with the CLI's own session logs as the offline fallback.

use std::path::PathBuf;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde_json::Value;

use super::credentials::{self, CodexLogin, Login, Secret};
use super::rollout;
use super::types::{
    unix_to_iso, window_from_minutes, Account, AccountId, AccountState, Credits, Window,
};

pub const DEFAULT_BASE: &str = "https://chatgpt.com";
const ID: AccountId = AccountId::Codex;
/// How long the provider gets to answer, matching the CLIs' own budget.
pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct CodexSource {
    pub base_url: String,
    pub timeout: Duration,
    pub home: Option<PathBuf>,
}

pub async fn account(client: &Client, source: &CodexSource) -> Account {
    let (token, account_id, expires_at) =
        match credentials::codex_login(source.home.as_deref()).await {
            Login::Found(CodexLogin::Chatgpt {
                access_token,
                account_id,
                expires_at,
            }) => (access_token, account_id, expires_at),
            Login::Found(CodexLogin::ApiKey) => {
                return Account::new(ID, AccountState::ApiKey).with_note(
                    "Codex is billed per token through an API key; there are no plan limits",
                )
            }
            Login::Missing => {
                return Account::new(ID, AccountState::SignedOut)
                    .with_note("Sign in with `codex login` to see usage")
            }
            Login::Unreadable(why) => return Account::error(ID, why),
        };
    let now = chrono::Utc::now().timestamp();
    if expires_at.is_some_and(|at| at <= now) {
        return with_fallback(Account::expired(ID), source).await;
    }
    match fetch(client, source, &token, account_id.as_deref()).await {
        Fetched::Report(account) => account,
        Fetched::Unauthorized => with_fallback(Account::expired(ID), source).await,
        Fetched::Failed(why) => with_fallback(Account::error(ID, why), source).await,
    }
}

enum Fetched {
    Report(Account),
    Unauthorized,
    Failed(String),
}

async fn fetch(
    client: &Client,
    source: &CodexSource,
    token: &Secret,
    account_id: Option<&str>,
) -> Fetched {
    let url = format!(
        "{}/backend-api/wham/usage",
        source.base_url.trim_end_matches('/')
    );
    let mut request = client
        .get(url)
        .bearer_auth(token.expose())
        .header(reqwest::header::USER_AGENT, "codex_cli_rs")
        .timeout(source.timeout);
    if let Some(account_id) = account_id {
        request = request.header("ChatGPT-Account-Id", account_id);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) if error.is_timeout() => {
            return Fetched::Failed("ChatGPT did not answer in time".into())
        }
        Err(_) => return Fetched::Failed("Could not reach ChatGPT".into()),
    };
    match response.status() {
        StatusCode::UNAUTHORIZED => return Fetched::Unauthorized,
        status if !status.is_success() => {
            return Fetched::Failed(format!("ChatGPT answered {}", status.as_u16()))
        }
        _ => {}
    }
    match response.json::<Value>().await {
        Ok(body) => Fetched::Report(parse(&body)),
        Err(_) => Fetched::Failed("ChatGPT sent an unreadable usage report".into()),
    }
}

pub fn parse(body: &Value) -> Account {
    let plan = body
        .get("plan_type")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut account = Account::new(ID, AccountState::Ok).with_plan(plan);
    let limits = body.get("rate_limit");
    account.windows = ["primary_window", "secondary_window"]
        .iter()
        .filter_map(|key| live_window(limits?.get(*key)?))
        .collect();
    account.credits = credits(body.get("credits"));
    account
}

fn live_window(window: &Value) -> Option<Window> {
    let used = window.get("used_percent")?.as_f64()?;
    let seconds = window
        .get("limit_window_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let reset = window
        .get("reset_at")
        .and_then(Value::as_i64)
        .and_then(unix_to_iso);
    let (id, label) = window_from_minutes(seconds / 60);
    Some(Window::new(id, label, used).resets_at(reset))
}

/// Shared with the rollout reader, whose `credits` object has the same shape.
pub fn credits(credits: Option<&Value>) -> Option<Credits> {
    let credits = credits.filter(|credits| credits.is_object())?;
    let balance = match credits.get("balance") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    };
    let enabled = credits
        .get("has_credits")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(Credits { balance, enabled })
}

/// The live call failed, so show what Codex itself last recorded — marked stale and
/// stamped with when it was recorded, never passed off as current.
async fn with_fallback(account: Account, source: &CodexSource) -> Account {
    let Some(home) = source.home.clone() else {
        return account;
    };
    let Some(snapshot) = rollout::latest(home).await else {
        return account;
    };
    let note = match account.note.as_deref() {
        Some(reason) => format!("{reason}. Showing the last usage Codex recorded."),
        None => "Showing the last usage Codex recorded.".to_string(),
    };
    let state = match account.state {
        AccountState::Error => AccountState::Ok,
        state => state,
    };
    Account {
        state,
        windows: snapshot.windows,
        credits: snapshot.credits.or(account.credits),
        note: Some(note),
        stale: true,
        fetched_at: snapshot.recorded_at,
        ..account
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
