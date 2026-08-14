//! Starting and steering a turn, with the permission mode it runs under.

use super::*;
use crate::mcp_agent_manager::fake_runner::{Harness, DIR};

/// The whole point of the argument: a session that may act without stopping on a prompt
/// nobody is there to answer.
#[tokio::test]
async fn a_started_session_is_configured_before_its_first_turn() {
    let harness = Harness::new();

    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "runner": "claude",
            "model": "haiku", "effort": "medium", "permission": "bypassPermissions",
            "message": "hello",
        }))
        .await
        .expect("a valid start");

    assert_eq!(started["permission"], "bypassPermissions");
    let session = started["session_id"].as_str().unwrap_or_default();
    let engine = harness
        .claude
        .engine_of(session)
        .await
        .expect("the session was configured");
    assert_eq!(engine.permission_mode.as_deref(), Some("bypassPermissions"));
    assert_eq!(engine.model.as_deref(), Some("haiku"));
    // And the turn itself carries it, for the engines that read the send body.
    let sends = harness.claude.sends().await;
    assert_eq!(sends[0].body["permission"], "bypassPermissions");
}

/// A session created with no opening message has no turn to carry the choices, so
/// recording them is the only chance to decide what its agent opens as.
#[tokio::test]
async fn a_session_started_without_a_message_is_still_configured() {
    let harness = Harness::new();

    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "runner": "claude",
            "model": "haiku", "effort": "medium", "permission": "acceptEdits",
        }))
        .await
        .expect("a valid start");

    assert_eq!(started["delivery"], "none");
    let session = started["session_id"].as_str().unwrap_or_default();
    let engine = harness.claude.engine_of(session).await.expect("configured");
    assert_eq!(engine.permission_mode.as_deref(), Some("acceptEdits"));
}

#[tokio::test]
async fn a_mode_the_runner_does_not_offer_creates_no_session() {
    let harness = Harness::new();

    let error = harness
        .call(json!({
            "op": "start", "directory": DIR, "runner": "opencode",
            "model": "gpt-5.6-luna", "effort": "low", "permission": "bypassPermissions",
        }))
        .await
        .expect_err("opencode has no such mode");

    assert!(format!("{error}").contains("build, plan"), "{error}");
    let listed = harness
        .call(json!({ "op": "list", "directory": DIR }))
        .await
        .expect("list");
    assert_eq!(
        listed["count"], 0,
        "a refused mode must leave nothing behind"
    );
}

/// Without a mode of its own, a child starts where its caller is rather than wherever its
/// agent happens to open — which was "ask about everything" on every runner.
#[tokio::test]
async fn a_child_inherits_the_callers_mode_when_none_is_named() {
    let harness = Harness::new();
    let parent = harness
        .session(RunnerKind::Claude, "bypassPermissions")
        .await;

    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "source_session": parent,
            "model": "haiku", "effort": "medium", "message": "go",
        }))
        .await
        .expect("a valid start");

    assert_eq!(started["permission"], "bypassPermissions");
    let sends = harness.claude.sends().await;
    assert_eq!(sends[0].body["permission"], "bypassPermissions");
}

#[tokio::test]
async fn a_child_is_told_how_to_report_its_final_work_to_its_caller() {
    let harness = Harness::new();
    let parent = harness
        .session(RunnerKind::Claude, "bypassPermissions")
        .await;

    harness
        .call(json!({
            "op": "start", "directory": DIR, "source_session": parent,
            "runner": "claude", "model": "haiku", "effort": "medium",
            "message": "Implement the requested change.",
        }))
        .await
        .expect("a valid start");

    let sends = harness.claude.sends().await;
    let opening = sends[0].text();
    assert!(opening.starts_with("Implement the requested change."));
    assert!(opening.contains(&parent));
    assert!(opening.contains("agent_send"));
    assert!(opening.contains("agent_runner_options"));
    assert!(opening.contains("files touched"));
    assert!(opening.contains("tests or checks"));
}

/// An absent mode is not a choice of "default": steering someone else's agent must not
/// quietly reconfigure it.
#[tokio::test]
async fn a_send_with_no_permission_leaves_the_targets_mode_alone() {
    let harness = Harness::new();
    let target = harness.session(RunnerKind::Claude, "acceptEdits").await;

    harness
        .call(json!({
            "op": "send", "directory": DIR, "target": target,
            "model": "haiku", "effort": "medium", "message": "carry on",
        }))
        .await
        .expect("a valid send");

    let sends = harness.claude.sends().await;
    assert!(
        sends[0].body.get("permission").is_none(),
        "{}",
        sends[0].body
    );
}

#[tokio::test]
async fn a_send_can_change_the_mode_the_target_runs_under() {
    let harness = Harness::new();
    let target = harness.session(RunnerKind::Claude, "default").await;

    harness
        .call(json!({
            "op": "send", "directory": DIR, "target": target,
            "model": "haiku", "effort": "medium", "permission": "bypassPermissions",
            "message": "stop asking",
        }))
        .await
        .expect("a valid send");

    let sends = harness.claude.sends().await;
    assert_eq!(sends[0].body["permission"], "bypassPermissions");
}
