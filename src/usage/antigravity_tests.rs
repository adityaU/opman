use std::time::Duration;

use super::*;
use crate::usage::types::AccountState;

const SAMPLE: &str = "### Session Usage\n- **Session total:** data not available yet\n\n### Quota Limits\n- **Gemini Models**\n  *(Models within this group: Gemini Flash, Gemini Pro)*\n  - **Weekly Limit Remaining:** 100% remaining (resets Oct 25, 2026, 12:17 UTC)\n  - **Five Hour Limit Remaining:** 40% remaining (resets Oct 18, 2026, 17:17 UTC)\n- **Claude and GPT models**\n  - **Weekly Limit Remaining:** 75% remaining (resets Oct 25, 2026, 12:17 UTC)\n- **Gemini 4 Argon**\n  *(For paid Pro plans, Argon quota is separate.)*\n  - **Gemini 4 Argon Weekly Limit:** 100% remaining (resets Oct 25, 2026, 12:17 UTC)";

#[test]
fn reads_every_limit_with_its_group() {
    let windows = parse_windows(SAMPLE);
    let labels: Vec<&str> = windows.iter().map(|w| w.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Gemini weekly",
            "Gemini 5-hour",
            "Claude and GPT weekly",
            "Gemini 4 Argon Weekly"
        ]
    );
}

#[test]
fn turns_remaining_into_used_and_reads_the_reset_time() {
    let windows = parse_windows(SAMPLE);
    assert_eq!(windows[1].used_percent, 60.0);
    assert_eq!(windows[1].resets_at.as_deref(), Some("2026-10-18T17:17:00Z"));
    assert_eq!(windows[2].used_percent, 25.0);
    assert_eq!(windows[0].used_percent, 0.0);
}

#[test]
fn text_without_limits_gives_no_windows() {
    assert!(parse_windows("### Session Usage\n- **Session total:** 5 tokens").is_empty());
    assert!(parse_windows("").is_empty());
}

/// A shell script that plays an ACP agent and answers `/usage` with `text`.
fn agent(text: &str) -> AgentConfig {
    let script = format!(
        r#"read a; printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{}}}}'
read b; printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"sessionId":"s","_meta":{{"antigravity":{{"tier":{{"name":"Google AI Pro"}}}}}}}}}}'
read c
printf '%s\n' '{{"jsonrpc":"2.0","method":"session/update","params":{{"update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"{text}"}}}}}}}}'
printf '%s\n' '{{"jsonrpc":"2.0","id":3,"result":{{}}}}'"#
    );
    AgentConfig {
        command: "sh".into(),
        args: vec!["-c".into(), script],
        ..AgentConfig::default()
    }
}

fn source(agent: AgentConfig) -> AntigravitySource {
    AntigravitySource {
        agent: Agent::Use(Box::new(agent)),
        timeout: Duration::from_secs(10),
    }
}

#[tokio::test]
async fn an_agent_that_reports_limits_gives_an_ok_account_with_its_plan() {
    let text = "- **Gemini Models**\\n  - **Weekly Limit Remaining:** 90% remaining (resets Oct 25, 2026, 12:17 UTC)";
    let account = account(&source(agent(text))).await.expect("enabled");
    assert_eq!(account.state, AccountState::Ok);
    assert_eq!(account.plan.as_deref(), Some("Google AI Pro"));
    assert_eq!(account.windows.len(), 1);
    assert_eq!(account.windows[0].used_percent, 10.0);
}

#[tokio::test]
async fn an_agent_with_no_limit_text_is_an_error_not_an_empty_ok() {
    let account = account(&source(agent("hello"))).await.expect("enabled");
    assert_eq!(account.state, AccountState::Error);
}

#[tokio::test]
async fn a_missing_download_asks_to_start_a_session_first() {
    let missing = AgentConfig {
        command: "/nonexistent/agy".into(),
        ..AgentConfig::default()
    };
    let account = account(&source(missing)).await.expect("enabled");
    assert_eq!(account.state, AccountState::SignedOut);
}

#[tokio::test]
async fn a_disabled_agent_has_no_account() {
    let off = AntigravitySource {
        agent: Agent::Off,
        ..AntigravitySource::default()
    };
    assert!(account(&off).await.is_none());
}

/// Needs a logged-in Antigravity: `cargo test usage::antigravity::antigravity_tests::live -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "starts the real Antigravity agent"]
async fn live() {
    let account = account(&AntigravitySource::default()).await.expect("enabled in acp.json");
    println!("{account:#?}");
    assert_eq!(account.state, AccountState::Ok);
    assert!(!account.windows.is_empty());
}
