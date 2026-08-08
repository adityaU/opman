//! Actually delivering a message: now, later, and when the runner will not take it.
//!
//! [`super::queue_tests`] covers parsing the mode. This covers what the mode then does,
//! which is where the failures that reach a calling agent as a hung tool call live.

use std::time::Duration;

use super::*;
use crate::mcp_agent_manager::fake_runner::{Harness, DIR};

fn message(target: &str, body: &str) -> QueuedMessage {
    QueuedMessage {
        id: "msg_test".into(),
        source: String::new(),
        target: target.into(),
        directory: DIR.into(),
        runner: None,
        body: json!({ "parts": [{ "type": "text", "text": body }] }),
    }
}

async fn session(harness: &Harness) -> String {
    harness
        .state
        .registry
        .create_session(RunnerKind::Opencode, DIR, "test")
        .await
        .expect("create")
        .id
}

#[tokio::test]
async fn an_immediate_send_reaches_the_runner_and_reports_where_it_landed() {
    let harness = Harness::new();
    let target = session(&harness).await;

    let result = deliver(
        &harness.state,
        message(&target, "now"),
        Some(Delivery::Immediate),
    )
    .await
    .expect("delivered");

    assert_eq!(result["delivery"], "immediate");
    assert_eq!(result["session_id"], target);
    assert_eq!(result["switched"], false);
    assert_eq!(harness.opencode.sends().await[0].text(), "now");
}

/// Queued means "not yet": nothing may reach the runner until the worker decides the
/// target is free.
#[tokio::test]
async fn a_queued_send_touches_nothing_until_the_worker_runs() {
    let harness = Harness::new();
    let target = session(&harness).await;

    let result = deliver(
        &harness.state,
        message(&target, "later"),
        Some(Delivery::Queued),
    )
    .await
    .expect("queued");

    assert_eq!(result["delivery"], "queued");
    assert_eq!(result["message_id"], "msg_test");
    assert!(harness.opencode.sends().await.is_empty());
    assert_eq!(
        harness.state.queues.lock().await[&target].len(),
        1,
        "the message should be waiting"
    );
}

#[tokio::test(start_paused = true)]
async fn the_worker_holds_a_queued_message_until_the_target_goes_idle() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_busy(true);
    deliver(
        &harness.state,
        message(&target, "when you're free"),
        Some(Delivery::Queued),
    )
    .await
    .expect("queued");
    let state = harness.state.clone();
    tokio::spawn(async move { worker(state).await });

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        harness.opencode.sends().await.is_empty(),
        "a busy target must not be steered"
    );

    harness.opencode.set_busy(false);
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert_eq!(harness.opencode.sends().await.len(), 1);
    assert!(harness.state.queues.lock().await[&target].is_empty());
}

/// The usual cause of a failed send is the target restarting, and the message is still
/// wanted once it is back — so it goes to the front, not the bin.
#[tokio::test(start_paused = true)]
async fn a_send_the_runner_refuses_goes_back_to_the_front_of_the_queue() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_failing(true);
    deliver(
        &harness.state,
        message(&target, "first"),
        Some(Delivery::Queued),
    )
    .await
    .expect("queued");
    let mut second = message(&target, "second");
    second.id = "msg_second".into();
    deliver(&harness.state, second, Some(Delivery::Queued))
        .await
        .expect("queued");
    let state = harness.state.clone();
    tokio::spawn(async move { worker(state).await });

    tokio::time::sleep(Duration::from_secs(2)).await;

    let queues = harness.state.queues.lock().await;
    let queued = &queues[&target];
    assert_eq!(queued.len(), 2, "nothing may be dropped");
    assert_eq!(queued[0].id, "msg_test", "order must survive the retry");
}

/// The failure this bound exists for: a runner that never answers used to hang the
/// *calling agent's tool call* forever, with no way to tell it why.
#[tokio::test(start_paused = true)]
async fn a_runner_that_never_answers_becomes_an_error_not_a_hang() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness
        .opencode
        .set_stall(Some(Duration::from_secs(600)))
        .await;

    let error = deliver(
        &harness.state,
        message(&target, "anyone there?"),
        Some(Delivery::Immediate),
    )
    .await
    .expect_err("the send should give up");

    let text = format!("{error}");
    assert!(text.contains(&target), "{text}");
    assert!(
        text.contains("agent_progress"),
        "the caller needs a next step: {text}"
    );
}

/// A runner switch forks a new session; the parent link has to follow it, or the child
/// loses the agent it was told to answer.
#[tokio::test]
async fn the_parent_link_follows_a_session_that_was_forked_by_a_runner_switch() {
    let harness = Harness::new();
    let target = session(&harness).await;
    let mut message = message(&target, "switch me");
    message.source = "ses_parent".into();
    message.runner = Some(RunnerKind::Claude);

    let result = deliver(&harness.state, message, Some(Delivery::Immediate))
        .await
        .expect("delivered");

    assert_eq!(result["switched"], true);
    let forked = result["session_id"].as_str().unwrap_or_default();
    assert_ne!(forked, target, "a switch forks a new session");
    assert_eq!(
        harness
            .state
            .parents
            .lock()
            .await
            .get(forked)
            .map(String::as_str),
        Some("ses_parent"),
    );
}

/// `{ ok: true }` is the only shape that means the turn started. A runner that hands back
/// its own failure must not be reported as a delivery.
#[test]
fn a_runners_own_failure_is_read_out_of_the_reply() {
    assert_eq!(runner_error(&json!({ "ok": true })), None);
    assert_eq!(
        runner_error(&json!({ "ok": false, "error": "no such session" })).as_deref(),
        Some("no such session"),
    );
    assert_eq!(
        runner_error(&json!({ "error": { "message": "Insufficient balance" } })).as_deref(),
        Some("Insufficient balance"),
    );
    // The shape live opencode returned when a turn died on a 401.
    assert_eq!(
        runner_error(&json!({ "error": { "name": "APIError", "data": { "message": "401" } } }))
            .as_deref(),
        Some("401"),
    );
}
