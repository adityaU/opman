use serde_json::json;

use super::*;
use crate::usage::fake_provider::{self, chatgpt_auth, claude_source, codex_source};
use crate::usage::types::AccountState;

fn service(base: &str, dir: &std::path::Path, claude_token: &str) -> UsageService {
    UsageService::new(Sources {
        claude: claude_source(base, &dir.join("claude"), claude_token, 4_102_444_800_000),
        codex: codex_source(base, &dir.join("codex"), Some(chatgpt_auth("codex"))),
    })
}

#[test]
fn runners_map_to_their_accounts() {
    let runners = [
        RunnerKind::Opencode,
        RunnerKind::ClaudeCode,
        RunnerKind::Claude,
        RunnerKind::Acp("codex".into()),
        RunnerKind::Acp("homegrown".into()),
    ];
    let mapped = runner_accounts(&runners);
    assert_eq!(
        serde_json::to_value(&mapped).unwrap(),
        json!({ "claude-code": "claude", "claude": "claude", "codex": "codex" })
    );
    assert_eq!(
        account_for(&RunnerKind::Acp("claude".into())),
        Some(AccountId::Claude)
    );
}

#[tokio::test]
async fn report_serializes_to_the_contract() {
    let (base, _fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let service = service(&base, dir.path(), "limits");
    let report = service
        .report(&Client::new(), &[RunnerKind::Claude], false)
        .await;
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["runners"], json!({ "claude": "claude" }));
    let claude = &value["accounts"][0];
    assert_eq!(claude["id"], "claude");
    assert_eq!(claude["label"], "Claude");
    assert_eq!(claude["state"], "ok");
    assert_eq!(claude["plan"], "max");
    assert_eq!(claude["stale"], false);
    assert_eq!(claude["windows"][0]["usedPercent"], 76.0);
    assert_eq!(claude["windows"][0]["resetsAt"], "2026-10-12T10:20:00Z");
    assert_eq!(claude["windows"][0]["severity"], "warning");
    assert_eq!(
        claude["credits"],
        json!({ "balance": "3750", "enabled": true })
    );
    assert!(claude["fetchedAt"]
        .as_str()
        .is_some_and(|at| at.ends_with('Z')));
    assert_eq!(value["accounts"][1]["id"], "codex");
    assert_eq!(value["accounts"][1]["state"], "ok");
    // No credential ever leaves in the report.
    let text = value.to_string();
    assert!(!text.contains("Bearer"));
    assert!(!text.contains("never-used"));
}

#[tokio::test]
async fn accounts_are_cached_and_refresh_bypasses() {
    let (base, fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let service = service(&base, dir.path(), "limits");
    let client = Client::new();
    service.accounts(&client, false).await;
    service.accounts(&client, false).await;
    assert_eq!((fake.claude_hits(), fake.codex_hits()), (1, 1));
    service.accounts(&client, true).await;
    assert_eq!((fake.claude_hits(), fake.codex_hits()), (2, 2));
}

#[tokio::test]
async fn concurrent_requests_share_one_fetch() {
    let (base, fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let service = service(&base, dir.path(), "limits");
    let client = Client::new();
    let (a, b) = tokio::join!(
        service.accounts(&client, false),
        service.accounts(&client, false)
    );
    assert_eq!(a, b);
    assert_eq!((fake.claude_hits(), fake.codex_hits()), (1, 1));
}

#[tokio::test]
async fn one_slow_account_does_not_hold_up_the_other() {
    let (base, _fake) = fake_provider::start().await;
    let dir = tempfile::tempdir().unwrap();
    let service = service(&base, dir.path(), "slow");
    let accounts = service.accounts(&Client::new(), false).await;
    assert_eq!(accounts[0].state, AccountState::Error);
    assert_eq!(accounts[1].state, AccountState::Ok);
}
