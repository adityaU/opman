//! The manager half, driven the way a child process drives it.
//!
//! The existing suites test the pieces in isolation — a schema, a parse, a flatten. These
//! cover the part that had none: dispatching a whole request against a registry, resolving
//! who a call is about, and the socket that carries it.

use super::fake_runner::{Harness, DIR};
use super::*;
use tokio::net::UnixListener;

#[tokio::test]
async fn every_operation_needs_a_project_except_the_auth_notice() {
    let harness = Harness::new();

    let error = harness
        .call(json!({ "op": "list" }))
        .await
        .expect_err("a project is required");
    assert!(format!("{error}").contains("project directory"), "{error}");

    // The proxy's login notice comes from outside any project and must still be accepted.
    let acknowledged = harness
        .call(json!({ "op": "mcp_auth_required", "server": "linear" }))
        .await
        .expect("a notice needs no project");
    assert_eq!(acknowledged["acknowledged"], true);
}

#[tokio::test]
async fn starting_creates_a_session_and_delivers_the_opening_message() {
    let harness = Harness::new();

    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "runner": "claude",
            "model": "haiku", "effort": "medium", "message": "hello",
        }))
        .await
        .expect("a valid start");

    assert_eq!(started["runner"], "claude");
    assert_eq!(started["delivery"], "immediate");
    let sends = harness.claude.sends().await;
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].text(), "hello");
    assert_eq!(sends[0].session, started["session_id"]);
    assert!(
        harness.opencode.sends().await.is_empty(),
        "the runner that was not named must not be sent to"
    );
}

/// A session with no opening turn is a legitimate thing to ask for.
#[tokio::test]
async fn starting_without_a_message_creates_the_session_and_sends_nothing() {
    let harness = Harness::new();

    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "model": "gpt-5.6-luna", "effort": "low",
        }))
        .await
        .expect("a valid start");

    assert_eq!(started["delivery"], "none");
    assert!(started["session_id"].is_string());
    assert!(harness.opencode.sends().await.is_empty());
}

/// The model is validated before the session exists, or a rejected dispatch leaves an
/// empty session behind that nobody asked for.
#[tokio::test]
async fn a_start_without_a_model_creates_nothing() {
    let harness = Harness::new();

    let error = harness
        .call(json!({ "op": "start", "directory": DIR, "message": "hi" }))
        .await
        .expect_err("model is required");

    assert!(
        format!("{error}").contains("'model' is required"),
        "{error}"
    );
    let listed = harness
        .call(json!({ "op": "list", "directory": DIR }))
        .await
        .expect("list");
    assert_eq!(listed["count"], 0, "no session should have been created");
}

/// Without the parent link a child cannot answer the agent that started it.
#[tokio::test]
async fn a_started_session_remembers_who_started_it() {
    let harness = Harness::new();
    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "source_session": "ses_parent",
            "model": "haiku", "effort": "medium",
        }))
        .await
        .expect("a valid start");
    let child = started["session_id"].as_str().unwrap_or_default();

    let parent = resolve_target(&harness.state, None, child)
        .await
        .expect("the parent should be known");

    assert_eq!(parent, "ses_parent");
}

#[tokio::test]
async fn a_named_target_wins_over_the_parent_and_the_word_parent_does_not() {
    let harness = Harness::new();
    harness
        .state
        .parents
        .lock()
        .await
        .insert("ses_child".into(), "ses_parent".into());

    let named = resolve_target(&harness.state, Some("ses_other"), "ses_child").await;
    assert_eq!(named.ok().as_deref(), Some("ses_other"));

    // "parent" is the schema's way of spelling the default, not a session id.
    let spelled = resolve_target(&harness.state, Some("parent"), "ses_child").await;
    assert_eq!(spelled.ok().as_deref(), Some("ses_parent"));

    let blank = resolve_target(&harness.state, Some("   "), "ses_child").await;
    assert_eq!(blank.ok().as_deref(), Some("ses_parent"));
}

#[tokio::test]
async fn a_call_with_no_target_and_no_parent_says_to_name_one() {
    let harness = Harness::new();

    let orphan = resolve_target(&harness.state, None, "")
        .await
        .expect_err("nothing to address");
    assert!(
        format!("{orphan}").contains("target is required"),
        "{orphan}"
    );

    let unknown = resolve_target(&harness.state, None, "ses_stranger")
        .await
        .expect_err("the parent was never recorded");
    assert!(
        format!("{unknown}").contains("explicit target"),
        "{unknown}"
    );
}

#[tokio::test]
async fn progress_reports_busy_state_and_the_queue_depth() {
    let harness = Harness::new();
    let started = harness
        .call(json!({
            "op": "start", "directory": DIR, "model": "gpt-5.6-luna", "effort": "low",
        }))
        .await
        .expect("a valid start");
    let target = started["session_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    harness.opencode.set_busy(true);
    harness
        .call(json!({
            "op": "send", "directory": DIR, "target": target, "delivery": "queued",
            "model": "gpt-5.6-luna", "effort": "low", "message": "later",
        }))
        .await
        .expect("a queued send");

    let progress = harness
        .call(json!({ "op": "progress", "directory": DIR, "target": target }))
        .await
        .expect("progress");

    assert_eq!(progress["busy"], true);
    assert_eq!(progress["queued_messages"], 1);
    assert_eq!(progress["runner"], "opencode");
}

/// The socket is the only way in from a child process, so its envelope is part of the
/// contract: `ok` decides whether the bridge reports a result or a tool error.
#[tokio::test]
async fn the_socket_answers_with_an_ok_envelope_on_both_paths() {
    let harness = Harness::new();
    let path = std::env::temp_dir().join(format!("opman-mgr-test-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind");
    let state = harness.state.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let state = state.clone();
            tokio::spawn(async move {
                let _ = handle_socket_connection(stream, state).await;
            });
        }
    });

    let good = exchange(&path, json!({ "op": "list", "directory": DIR })).await;
    assert_eq!(good["ok"], true);
    assert_eq!(good["data"]["count"], 0);

    let bad = exchange(&path, json!({ "op": "teleport", "directory": DIR })).await;
    assert_eq!(bad["ok"], false);
    assert!(
        bad["error"]
            .as_str()
            .unwrap_or_default()
            .contains("teleport"),
        "{bad}"
    );
    let _ = std::fs::remove_file(&path);
}

async fn exchange(path: &std::path::Path, request: Value) -> Value {
    let mut stream = UnixStream::connect(path).await.expect("connect");
    let line = format!("{request}\n");
    stream.write_all(line.as_bytes()).await.expect("write");
    stream.shutdown().await.expect("shutdown");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .await
        .expect("read");
    serde_json::from_str(response.trim()).expect("a json envelope")
}
