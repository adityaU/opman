use serde_json::json;

use super::*;
use crate::usage::fake_provider::{
    self, chatgpt_auth, codex_body, codex_source, jwt, rollout_line, write_rollout,
};

#[test]
fn parses_windows_plan_and_credits() {
    let account = parse(&codex_body());
    assert_eq!(account.state, AccountState::Ok);
    assert_eq!(account.plan.as_deref(), Some("plus"));
    let names: Vec<_> = account
        .windows
        .iter()
        .map(|w| (w.id.as_str(), w.label.as_str()))
        .collect();
    assert_eq!(names, [("session", "5-hour"), ("weekly", "Weekly")]);
    assert_eq!(account.windows[0].used_percent, 60.0);
    assert_eq!(
        account.windows[0].resets_at.as_deref(),
        Some("2026-10-12T10:27:21Z")
    );
    let credits = account.credits.expect("credits");
    assert_eq!(credits.balance.as_deref(), Some("0"));
    assert!(!credits.enabled);
}

#[test]
fn missing_rate_limit_means_no_windows() {
    let account = parse(&json!({ "plan_type": "free" }));
    assert!(account.windows.is_empty());
    assert_eq!(account.credits, None);
}

#[tokio::test]
async fn live_report() {
    let (base, fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    let account = account(
        &Client::new(),
        &codex_source(&base, home.path(), Some(chatgpt_auth("codex"))),
    )
    .await;
    assert_eq!(account.state, AccountState::Ok);
    assert!(!account.stale);
    assert_eq!(account.windows.len(), 2);
    assert_eq!(fake.codex_hits(), 1);
}

#[tokio::test]
async fn api_key_mode_has_no_windows_and_no_call() {
    let (base, fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    let auth = json!({ "auth_mode": "apikey", "OPENAI_API_KEY": "sk-test", "tokens": null });
    let account = account(
        &Client::new(),
        &codex_source(&base, home.path(), Some(auth)),
    )
    .await;
    assert_eq!(account.state, AccountState::ApiKey);
    assert!(account.windows.is_empty());
    assert_eq!(fake.codex_hits(), 0);
}

#[tokio::test]
async fn expired_jwt_is_reported_without_a_network_call() {
    let (base, fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    let source = codex_source(&base, home.path(), Some(chatgpt_auth(&jwt(1))));
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Expired);
    assert!(account.note.unwrap_or_default().contains("`codex`"));
    assert_eq!(fake.codex_hits(), 0);
}

#[tokio::test]
async fn unauthorized_is_expired() {
    let (base, _fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    let source = codex_source(&base, home.path(), Some(chatgpt_auth("unauthorized")));
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Expired);
    assert!(!account.stale);
}

#[tokio::test]
async fn timeout_without_logs_is_an_error() {
    let (base, _fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    let source = codex_source(&base, home.path(), Some(chatgpt_auth("slow")));
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Error);
    assert_eq!(
        account.note.as_deref(),
        Some("ChatGPT did not answer in time")
    );
}

#[tokio::test]
async fn failed_call_falls_back_to_the_rollout_marked_stale() {
    let (base, _fake) = fake_provider::start().await;
    let home = tempfile::tempdir().unwrap();
    write_rollout(
        home.path(),
        "2026/10/04",
        "rollout-2026-10-04T16-51-06-a.jsonl",
        &[rollout_line("2026-10-04T16:51:34.608Z", 1.0)],
    );
    let source = codex_source(&base, home.path(), Some(chatgpt_auth("boom")));
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Ok);
    assert!(account.stale);
    assert_eq!(account.fetched_at, "2026-10-04T16:51:34Z");
    assert_eq!(account.windows.len(), 2);
    assert_eq!(account.windows[0].used_percent, 1.0);
    let note = account.note.unwrap_or_default();
    assert!(note.starts_with("ChatGPT answered 500. "), "{note}");
}

#[tokio::test]
async fn missing_login_is_signed_out() {
    let home = tempfile::tempdir().unwrap();
    let source = codex_source("http://127.0.0.1:9", home.path(), None);
    assert_eq!(
        account(&Client::new(), &source).await.state,
        AccountState::SignedOut
    );
}
