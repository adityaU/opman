//! Real runners, real models: starting an agent, waiting on it, reaching its parent,
//! stopping it.
//!
//! Setup and the socket client live in [`super::live_support`]; the catalogue tests in
//! [`super::live_options`].
//!
//! Run with:
//! ```text
//! OPMAN_LIVE_AGENT_TESTS=1 cargo test --bin opman mcp_agent_manager::live -- --ignored --nocapture
//! ```

use std::time::Duration;

use serde_json::json;

use super::live_support::{
    call, directory, enabled, round_trip, start, CLAUDE_MODEL, CLAUDE_SENTINEL, LUNA_MODEL,
    LUNA_PROVIDER, LUNA_SENTINEL,
};

#[tokio::test]
#[ignore = "spends real tokens; needs a running opman"]
async fn claude_haiku_answers_a_started_session() {
    if !enabled() {
        return;
    }
    round_trip("claude", CLAUDE_MODEL, None, CLAUDE_SENTINEL).await;
}

#[tokio::test]
#[ignore = "spends real tokens; needs a running opman"]
async fn opencode_luna_answers_a_started_session() {
    if !enabled() {
        return;
    }
    round_trip("opencode", LUNA_MODEL, Some(LUNA_PROVIDER), LUNA_SENTINEL).await;
}

/// The A2A round trip, driven the way a child's bridge drives it.
///
/// The child half of `agent_send` is a socket line carrying `source_session` and no
/// target; the manager is supposed to look the parent up and deliver. Reproducing it here
/// rather than asking a model to call the tool keeps the assertion about the manager
/// instead of about whether a model followed instructions.
#[tokio::test]
#[ignore = "spends real tokens; needs a running opman"]
async fn a_child_can_reach_the_parent_that_started_it() {
    if !enabled() {
        return;
    }
    let parent = round_trip("claude", CLAUDE_MODEL, None, CLAUDE_SENTINEL).await;
    let started = call(json!({
        "op": "start", "directory": directory(), "runner": "claude",
        "source_session": parent, "model": CLAUDE_MODEL, "effort": "low",
        "title": "live a2a child",
    }))
    .await;
    let child = started["session_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    // Exactly the line the child's bridge writes for `agent_send` with no `to`.
    let sent = call(json!({
        "op": "send", "directory": directory(), "source_session": child,
        "model": CLAUDE_MODEL, "effort": "low",
        "message": "Reply with exactly one word: CHILD-PING-SEEN",
    }))
    .await;

    assert_eq!(sent["delivery"], "immediate");
    assert_eq!(
        sent["session_id"], parent,
        "the message must land in the parent, not anywhere else",
    );
    let progress = call(json!({
        "op": "progress", "directory": directory(), "target": parent,
    }))
    .await;
    assert!(
        progress.to_string().contains("CHILD-PING-SEEN"),
        "the parent's transcript should hold the child's message",
    );
}

/// A wedged agent has to be stoppable, or the only remedy is restarting opman.
#[tokio::test]
#[ignore = "spends real tokens; needs a running opman"]
async fn a_running_agent_can_be_aborted_and_appears_in_the_listing() {
    if !enabled() {
        return;
    }
    let started = call(start(
        "claude",
        CLAUDE_MODEL,
        None,
        "Count slowly from 1 to 500, one number per line.",
    ))
    .await;
    let target = started["session_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let listed = call(json!({ "op": "list", "directory": directory() })).await;
    assert!(
        listed["agents"]
            .as_array()
            .is_some_and(|agents| agents.iter().any(|agent| agent["agent_id"] == target)),
        "a session the manager started must be listed",
    );

    let aborted = call(json!({ "op": "abort", "directory": directory(), "target": target })).await;
    assert_eq!(aborted["aborted"], true);

    tokio::time::sleep(Duration::from_secs(3)).await;
    let progress =
        call(json!({ "op": "progress", "directory": directory(), "target": target })).await;
    assert_eq!(
        progress["busy"], false,
        "the turn should have been cancelled"
    );
}
