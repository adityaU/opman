//! Mock-upstream success-path tests for `session_handlers.rs` (wave 3).
//!
//! The earlier sibling test files cover the pure mapping helpers and the
//! upstream-unreachable (5xx) branches. These tests instead stand up a mock
//! "opencode" HTTP server via [`start_mock_upstream`], point `base_url()` at it
//! with [`scope_base_url`], and drive the **success** path of every proxy
//! handler end-to-end through the real router — plus the upstream 4xx/5xx and
//! `.json()` decode-error branches that only run when a server actually
//! responds. This is where the bulk of previously-missed lines live.

use super::*;
use axum::http::StatusCode;
use axum::routing::{delete, get, patch, post};
use axum::Router;
use crate::web::test_support::{
    scope_base_url, send_json, start_mock_upstream, test_router, test_server_state,
};
use crate::web::types::ServerState;
use serde_json::json;

/// Redirect config/state writes to a throwaway temp dir (once per process) so
/// `add_project`'s `Config::save` never touches the real user config.
fn isolate_env() {
    use std::sync::OnceLock;
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let _env_guard = crate::claude_engine::claude_cli::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", d.path());
        std::env::set_var("XDG_STATE_HOME", d.path());
        d
    });
}

/// A server state with one active project so `resolve_project_dir` succeeds.
async fn state_with_project() -> (ServerState, tempfile::TempDir) {
    isolate_env();
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), Some("proj"))
        .await
        .expect("add project");
    (state, tmp)
}

/// Run a request through the router with `base_url()` scoped to `mock`.
async fn drive(
    mock: Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let (state, _tmp) = state_with_project().await;
    let base = start_mock_upstream(mock).await;
    let router = test_router(state);
    let (status, bytes) =
        scope_base_url(base, send_json(router, method, uri, body)).await;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

// ── get_session_messages ─────────────────────────────────────────────

#[tokio::test]
async fn get_messages_success_maps_and_paginates() {
    let mock = Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(json!([
                { "info": { "role": "assistant", "time": { "created": 2 } }, "parts": [] },
                { "info": { "role": "user", "time": { "created": 1 } }, "parts": [] }
            ]))
        }),
    );
    let (status, v) = drive(mock, "GET", "/api/session/s1/messages", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["total"], 2);
    assert_eq!(v["has_more"], false);
    let msgs = v["messages"].as_array().unwrap();
    // Sorted ascending by created: user(1) then assistant(2).
    assert_eq!(msgs[0].pointer("/info/time/created").unwrap(), 1);
    assert_eq!(msgs[1].pointer("/info/time/created").unwrap(), 2);
}

#[tokio::test]
async fn get_messages_success_with_limit_query() {
    let mock = Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(json!([
                { "info": { "time": { "created": 1 } } },
                { "info": { "time": { "created": 2 } } },
                { "info": { "time": { "created": 3 } } }
            ]))
        }),
    );
    let (status, v) = drive(mock, "GET", "/api/session/s1/messages?limit=1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["total"], 3);
    assert_eq!(v["has_more"], true);
    assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    assert_eq!(v["messages"][0].pointer("/info/time/created").unwrap(), 3);
}

#[tokio::test]
async fn get_messages_object_body_normalised() {
    let mock = Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(json!({
                "m1": { "info": { "time": { "created": 5 } } }
            }))
        }),
    );
    let (status, v) = drive(mock, "GET", "/api/session/s1/messages", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["total"], 1);
}

#[tokio::test]
async fn get_messages_invalid_json_is_parse_error() {
    // Upstream replies 200 but with a non-JSON body → resp.json() fails →
    // WebError::Internal("Parse error…") → 5xx. Covers the decode-error branch.
    let mock = Router::new().route(
        "/session/{id}/message",
        get(|| async { "<<< definitely not json >>>" }),
    );
    let (status, _v) = drive(mock, "GET", "/api/session/s1/messages", None).await;
    assert!(status.is_server_error(), "got {status}");
}

// ── send_message ─────────────────────────────────────────────────────

#[tokio::test]
async fn send_message_success_relays_body() {
    let mock = Router::new().route(
        "/session/{id}/message",
        post(|| async { axum::Json(json!({ "id": "msg_out", "ok": true })) }),
    );
    let (status, v) = drive(
        mock,
        "POST",
        "/api/session/s1/message",
        Some(json!({ "parts": [{ "type": "text", "text": "hi" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["id"], "msg_out");
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn send_message_upstream_4xx_is_error() {
    let mock = Router::new().route(
        "/session/{id}/message",
        post(|| async {
            (StatusCode::BAD_REQUEST, axum::Json(json!({ "error": "bad model" })))
        }),
    );
    let (status, _v) = drive(
        mock,
        "POST",
        "/api/session/s1/message",
        Some(json!({ "parts": [] })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

// ── abort_session ────────────────────────────────────────────────────

#[tokio::test]
async fn abort_session_success_returns_ok() {
    let mock = Router::new().route(
        "/session/{id}/abort",
        post(|| async { StatusCode::OK }),
    );
    let (status, _v) = drive(mock, "POST", "/api/session/s1/abort", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn abort_session_upstream_5xx_is_error() {
    let mock = Router::new().route(
        "/session/{id}/abort",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
    );
    let (status, _v) = drive(mock, "POST", "/api/session/s1/abort", None).await;
    assert!(status.is_server_error(), "got {status}");
}

// ── queue: get / clear / remove ──────────────────────────────────────

#[tokio::test]
async fn get_queue_success_relays_array() {
    let mock = Router::new().route(
        "/session/{id}/queue",
        get(|| async { axum::Json(json!(["prompt a", "prompt b"])) }),
    );
    let (status, v) = drive(mock, "GET", "/api/session/s1/queue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!(["prompt a", "prompt b"]));
}

#[tokio::test]
async fn clear_queue_success_relays_body() {
    let mock = Router::new().route(
        "/session/{id}/queue",
        delete(|| async { axum::Json(json!({ "cleared": 2 })) }),
    );
    let (status, v) = drive(mock, "DELETE", "/api/session/s1/queue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["cleared"], 2);
}

#[tokio::test]
async fn remove_queue_item_success_relays_body() {
    let mock = Router::new().route(
        "/session/{id}/queue/{index}",
        delete(|| async { axum::Json(json!({ "removed": 3 })) }),
    );
    let (status, v) = drive(mock, "DELETE", "/api/session/s1/queue/3", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["removed"], 3);
}

#[tokio::test]
async fn queue_upstream_error_surfaces_5xx() {
    let mock = Router::new().route(
        "/session/{id}/queue",
        get(|| async {
            (StatusCode::NOT_FOUND, axum::Json(json!({ "message": "no session" })))
        }),
    );
    let (status, _v) = drive(mock, "GET", "/api/session/s1/queue", None).await;
    assert!(status.is_server_error(), "got {status}");
}

// ── delete_session ───────────────────────────────────────────────────

#[tokio::test]
async fn delete_session_success_returns_ok() {
    let mock = Router::new().route(
        "/session/{id}",
        delete(|| async { StatusCode::OK }),
    );
    let (status, _v) = drive(mock, "DELETE", "/api/session/s1", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn delete_session_upstream_error_reads_body() {
    let mock = Router::new().route(
        "/session/{id}",
        delete(|| async {
            (StatusCode::FORBIDDEN, axum::Json(json!({ "message": "denied" })))
        }),
    );
    let (status, _v) = drive(mock, "DELETE", "/api/session/s1", None).await;
    assert!(status.is_server_error(), "got {status}");
}

// ── rename_session ───────────────────────────────────────────────────

#[tokio::test]
async fn rename_session_success_relays_body() {
    let mock = Router::new().route(
        "/session/{id}",
        patch(|| async { axum::Json(json!({ "title": "new title" })) }),
    );
    let (status, v) = drive(
        mock,
        "PATCH",
        "/api/session/s1",
        Some(json!({ "title": "new title" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["title"], "new title");
}

#[tokio::test]
async fn rename_session_upstream_error() {
    let mock = Router::new().route(
        "/session/{id}",
        patch(|| async {
            (StatusCode::BAD_REQUEST, axum::Json(json!({ "message": "bad title" })))
        }),
    );
    let (status, _v) = drive(
        mock,
        "PATCH",
        "/api/session/s1",
        Some(json!({ "title": "x" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

// ── execute_command ──────────────────────────────────────────────────

#[tokio::test]
async fn execute_command_success_relays_body() {
    let mock = Router::new().route(
        "/session/{id}/command",
        post(|| async { axum::Json(json!({ "output": "compacted" })) }),
    );
    let (status, v) = drive(
        mock,
        "POST",
        "/api/session/s1/command",
        Some(json!({ "command": "/compact", "arguments": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["output"], "compacted");
}

#[tokio::test]
async fn execute_command_upstream_error_preserves_status() {
    // Upstream 404 → CommandError → WebError::Upstream(404, ..) → 404 relayed.
    let mock = Router::new().route(
        "/session/{id}/command",
        post(|| async {
            (StatusCode::NOT_FOUND, axum::Json(json!({ "error": "no such command" })))
        }),
    );
    let (status, _v) = drive(
        mock,
        "POST",
        "/api/session/s1/command",
        Some(json!({ "command": "/nope", "arguments": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── providers / commands ─────────────────────────────────────────────

#[tokio::test]
async fn get_providers_success() {
    let mock = Router::new().route(
        "/provider",
        get(|| async { axum::Json(json!({ "providers": ["anthropic"] })) }),
    );
    let (status, v) = drive(mock, "GET", "/api/providers", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["providers"][0], "anthropic");
}

#[tokio::test]
async fn get_commands_success() {
    let mock = Router::new().route(
        "/command",
        get(|| async { axum::Json(json!([{ "name": "/compact" }])) }),
    );
    let (status, v) = drive(mock, "GET", "/api/commands", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v[0]["name"], "/compact");
}

// ── reply_permission / reply_question ────────────────────────────────

#[tokio::test]
async fn reply_permission_success_returns_ok() {
    let mock = Router::new().route(
        "/permission/{id}/reply",
        post(|| async { StatusCode::OK }),
    );
    let (status, _v) = drive(
        mock,
        "POST",
        "/api/permission/req1/reply",
        Some(json!({ "reply": "once" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn reply_question_success_returns_ok() {
    let mock = Router::new().route(
        "/question/{id}/reply",
        post(|| async { StatusCode::OK }),
    );
    let (status, _v) = drive(
        mock,
        "POST",
        "/api/question/req1/reply",
        Some(json!({ "answers": [["yes"]] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── a2ui_callback ────────────────────────────────────────────────────

#[tokio::test]
async fn a2ui_callback_success_returns_ok_true() {
    let mock = Router::new().route(
        "/session/{id}/message",
        post(|| async { StatusCode::OK }),
    );
    let (status, v) = drive(
        mock,
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb1", "payload": { "field": "v" } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({ "ok": true }));
}

#[tokio::test]
async fn a2ui_callback_null_payload_success() {
    let mock = Router::new().route(
        "/session/{id}/message",
        post(|| async { StatusCode::OK }),
    );
    let (status, v) = drive(
        mock,
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb2" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({ "ok": true }));
}

#[tokio::test]
async fn a2ui_callback_upstream_error() {
    let mock = Router::new().route(
        "/session/{id}/message",
        post(|| async {
            (StatusCode::BAD_GATEWAY, axum::Json(json!({ "message": "down" })))
        }),
    );
    let (status, _v) = drive(
        mock,
        "POST",
        "/api/session/s1/a2ui/callback",
        Some(json!({ "callback_id": "cb3" })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}
