use super::*;
use crate::usage::fake_provider::{self, claude_source, legacy_body, limits_body};

const FAR_FUTURE_MS: i64 = 4_102_444_800_000; // 2100-01-01

#[test]
fn limits_are_preferred_and_named() {
    let account = parse(&limits_body());
    assert_eq!(account.state, AccountState::Ok);
    let ids: Vec<_> = account.windows.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["session", "weekly", "weekly_opus"]);
    let labels: Vec<_> = account.windows.iter().map(|w| w.label.as_str()).collect();
    assert_eq!(labels, ["5-hour", "Weekly", "Weekly · Opus"]);
    assert_eq!(account.windows[0].used_percent, 76.0);
    assert_eq!(account.windows[0].severity, Some(Severity::Warning));
    assert_eq!(
        account.windows[0].resets_at.as_deref(),
        Some("2026-10-12T10:20:00Z")
    );
    // An unknown severity word falls back to one derived from the percentage.
    assert_eq!(account.windows[2].severity, Some(Severity::Critical));
    assert_eq!(account.windows[2].resets_at, None);
}

#[test]
fn legacy_fields_are_the_fallback() {
    let account = parse(&legacy_body());
    let ids: Vec<_> = account.windows.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["session", "weekly", "weekly_opus"]);
    assert_eq!(account.windows[1].used_percent, 15.0);
    assert_eq!(account.windows[1].severity, Some(Severity::Normal));
    assert_eq!(account.windows[2].label, "Weekly · Opus");
    // An unparseable timestamp is dropped, not passed through.
    assert_eq!(account.windows[2].resets_at, None);
    let credits = account.credits.expect("extra_usage is reported");
    assert!(!credits.enabled);
    assert_eq!(credits.balance, None);
}

#[test]
fn empty_limits_list_uses_legacy_fields() {
    let mut body = legacy_body();
    body["limits"] = serde_json::json!([]);
    assert_eq!(parse(&body).windows.len(), 3);
}

#[test]
fn extra_usage_balance_is_what_remains() {
    let credits = parse(&limits_body()).credits.expect("credits");
    assert!(credits.enabled);
    assert_eq!(credits.balance.as_deref(), Some("3750"));
}

#[test]
fn unknown_limit_kinds_get_readable_names() {
    assert_eq!(name_kind("weekly_sonnet").1, "Weekly · Sonnet");
    assert_eq!(name_kind("monthly_all").1, "Monthly All");
}

#[tokio::test]
async fn live_report_carries_the_plan() {
    let (base, fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let account = account(
        &Client::new(),
        &claude_source(&base, dir.path(), "limits", FAR_FUTURE_MS),
    )
    .await;
    assert_eq!(account.state, AccountState::Ok);
    assert_eq!(account.plan.as_deref(), Some("max"));
    assert_eq!(account.windows.len(), 3);
    assert_eq!(fake.claude_hits(), 1);
}

#[tokio::test]
async fn expired_login_is_reported_without_a_network_call() {
    let (base, fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let account = account(
        &Client::new(),
        &claude_source(&base, dir.path(), "limits", 1),
    )
    .await;
    assert_eq!(account.state, AccountState::Expired);
    assert!(account.note.unwrap_or_default().contains("`claude`"));
    assert_eq!(fake.claude_hits(), 0);
}

#[tokio::test]
async fn unauthorized_is_expired() {
    let (base, _fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let source = claude_source(&base, dir.path(), "unauthorized", FAR_FUTURE_MS);
    assert_eq!(
        account(&Client::new(), &source).await.state,
        AccountState::Expired
    );
}

#[tokio::test]
async fn slow_provider_is_an_error() {
    let (base, _fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let source = claude_source(&base, dir.path(), "slow", FAR_FUTURE_MS);
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Error);
    assert_eq!(
        account.note.as_deref(),
        Some("Anthropic did not answer in time")
    );
}

#[tokio::test]
async fn server_error_is_an_error() {
    let (base, _fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let source = claude_source(&base, dir.path(), "boom", FAR_FUTURE_MS);
    let account = account(&Client::new(), &source).await;
    assert_eq!(account.state, AccountState::Error);
    assert_eq!(account.note.as_deref(), Some("Anthropic answered 500"));
}

#[tokio::test]
async fn missing_login_is_signed_out() {
    let dir = tempfile::tempdir().unwrap();
    let source = ClaudeSource {
        base_url: "http://127.0.0.1:9".into(),
        timeout: fake_provider::TEST_TIMEOUT,
        config_dir: Some(dir.path().to_path_buf()),
        keychain: false,
    };
    assert_eq!(
        account(&Client::new(), &source).await.state,
        AccountState::SignedOut
    );
}
