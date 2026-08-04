//! Generated tests for `session_handlers.rs`.
//!
//! Proxy endpoints call the opencode server through `http_client`; with no
//! server running these fail fast with connection-refused, so we assert the
//! handler surfaces a 5xx (it still executed — that is the coverage we want).
//! Local endpoints (`get_pending`, `mark_session_seen`) and the pure
//! `paginate_messages` helper are asserted for real success behaviour.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use serde_json::json;

/// Point the global upstream base-url at an unreachable loopback port so proxy
/// requests fail fast (connection refused) instead of panicking.
fn init_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

/// Redirect config/state writes to a throwaway temp dir (once per process) so
/// `add_project`'s `Config::save` never touches the real user config.
fn isolate_env() {
    use std::sync::OnceLock;
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let _env_guard = crate::claude_engine::claude_cli::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", d.path());
        std::env::set_var("XDG_STATE_HOME", d.path());
        d
    });
}

/// A server state with one active project (so `resolve_project_dir` succeeds)
/// and the upstream base-url initialised.
async fn state_with_project() -> (ServerState, tempfile::TempDir) {
    isolate_env();
    init_base_url();
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), Some("proj"))
        .await
        .expect("add project");
    (state, tmp)
}

// ── Proxy endpoints: upstream unreachable → 5xx ─────────────────────

#[tokio::test]
async fn get_messages_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "GET", "/api/session/s1/messages", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn get_messages_with_pagination_query_parses() {
    // Exercises MessagePageQuery deserialisation (limit + before).
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "GET",
        "/api/session/s1/messages?limit=5&before=1000",
        None,
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn get_messages_no_project_is_bad_request() {
    init_base_url();
    let state = test_server_state();
    let (status, _) = send_json(test_router(state), "GET", "/api/session/s1/messages", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_message_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/message",
        Some(json!({ "parts": [{ "type": "text", "text": "hi" }] })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn abort_session_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "POST", "/api/session/s1/abort", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn get_queue_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "GET", "/api/session/s1/queue", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn clear_queue_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "DELETE", "/api/session/s1/queue", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn remove_queue_item_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "DELETE",
        "/api/session/s1/queue/3",
        None,
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn delete_session_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "DELETE", "/api/session/s1", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn rename_session_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "PATCH",
        "/api/session/s1",
        Some(json!({ "title": "renamed" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn execute_command_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/command",
        Some(json!({ "command": "/compact", "arguments": "" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn get_providers_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "GET", "/api/providers", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn get_commands_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(test_router(state), "GET", "/api/commands", None).await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn reply_permission_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/permission/req1/reply",
        Some(json!({ "reply": "once" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn reply_question_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/question/req1/reply",
        Some(json!({ "answers": [["yes"]] })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn a2ui_callback_null_payload_upstream_error() {
    // payload omitted → the "[A2UI callback: ...]" text branch.
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb1" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

#[tokio::test]
async fn a2ui_callback_with_payload_upstream_error() {
    // non-empty payload → the fenced-json text branch.
    let (state, _tmp) = state_with_project().await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb1", "payload": { "field": "value" } })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

// ── Local endpoints: real success behaviour ─────────────────────────

#[tokio::test]
async fn get_pending_returns_empty_collections() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/pending", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["permissions"].is_array());
    assert!(v["questions"].is_array());
    assert_eq!(v["permissions"].as_array().unwrap().len(), 0);
    assert_eq!(v["questions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn mark_session_seen_returns_ok() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/s1/mark_seen",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── paginate_messages (pure) ────────────────────────────────────────

fn msg(created: u64) -> serde_json::Value {
    json!({ "info": { "time": { "created": created } } })
}

fn created_of(m: &serde_json::Value) -> u64 {
    m.pointer("/info/time/created")
        .and_then(|v| v.as_u64())
        .unwrap()
}

#[test]
fn paginate_array_no_pagination_sorts_ascending() {
    let body = json!([msg(3), msg(1), msg(2)]);
    let out = paginate_messages(body, 0, None);
    assert_eq!(out["total"], 3);
    assert_eq!(out["has_more"], false);
    let msgs = out["messages"].as_array().unwrap();
    assert_eq!(created_of(&msgs[0]), 1);
    assert_eq!(created_of(&msgs[2]), 3);
}

#[test]
fn paginate_object_input_is_normalised() {
    let body = json!({ "a": msg(2), "b": msg(1) });
    let out = paginate_messages(body, 0, None);
    assert_eq!(out["total"], 2);
    assert_eq!(out["messages"].as_array().unwrap().len(), 2);
}

#[test]
fn paginate_non_container_yields_empty() {
    let out = paginate_messages(json!(42), 0, None);
    assert_eq!(out["total"], 0);
    assert_eq!(out["messages"].as_array().unwrap().len(), 0);
    assert_eq!(out["has_more"], false);
}

#[test]
fn paginate_limit_takes_most_recent() {
    let body = json!([msg(1), msg(2), msg(3), msg(4), msg(5)]);
    let out = paginate_messages(body, 2, None);
    assert_eq!(out["total"], 5);
    assert_eq!(out["has_more"], true);
    let msgs = out["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(created_of(&msgs[0]), 4);
    assert_eq!(created_of(&msgs[1]), 5);
}

#[test]
fn paginate_before_filters_without_limit() {
    let body = json!([msg(1), msg(2), msg(3)]);
    let out = paginate_messages(body, 0, Some(3));
    // total counts everything; filtered keeps created < 3.
    assert_eq!(out["total"], 3);
    assert_eq!(out["has_more"], false);
    let msgs = out["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(created_of(&msgs[1]), 2);
}

#[test]
fn paginate_limit_and_before_combined() {
    let body = json!([msg(1), msg(2), msg(3), msg(4), msg(5)]);
    let out = paginate_messages(body, 2, Some(5));
    // before=5 keeps 1..4, limit 2 keeps the last two (3,4).
    assert_eq!(out["total"], 5);
    assert_eq!(out["has_more"], true);
    let msgs = out["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(created_of(&msgs[0]), 3);
    assert_eq!(created_of(&msgs[1]), 4);
}

#[test]
fn paginate_limit_greater_than_count_no_more() {
    let body = json!([msg(1), msg(2)]);
    let out = paginate_messages(body, 5, None);
    assert_eq!(out["has_more"], false);
    assert_eq!(out["messages"].as_array().unwrap().len(), 2);
}
