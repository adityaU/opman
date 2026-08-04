//! Mock-upstream tests for the opencode-proxy paths in `context_handlers.rs`.
//!
//! Pre-existing tests only cover the no-project / upstream-down branches (which
//! fall back to the default context limit). Here a mock opencode server drives
//! the **success** paths: `get_session_todos` parses the `/session/{id}/todo`
//! array, and `get_context_window` resolves the real max context window from a
//! mock `/provider` payload (both the `{all:[…]}` and flat-array shapes).

use crate::web::test_support::{
    scope_base_url, send_json, start_mock_upstream, test_router, test_server_state,
};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::json;

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

async fn state_with_project() -> (ServerState, tempfile::TempDir) {
    isolate_env();
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), None)
        .await
        .expect("add project");
    (state, tmp)
}

// ── get_session_todos success ───────────────────────────────────────

#[tokio::test]
async fn get_session_todos_success_returns_parsed_list() {
    let mock = axum::Router::new().route(
        "/session/{id}/todo",
        get(|| async {
            axum::Json(json!([
                { "content": "first", "status": "pending", "priority": "high" },
                { "content": "second", "status": "completed", "priority": "low" }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/session/s1/todos", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["content"], "first");
    assert_eq!(arr[1]["status"], "completed");
}

#[tokio::test]
async fn get_session_todos_malformed_upstream_is_500() {
    // Upstream 200 but items are missing required fields → deserialise error.
    let mock = axum::Router::new().route(
        "/session/{id}/todo",
        get(|| async { axum::Json(json!([{ "unexpected": true }])) }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "GET", "/api/session/s1/todos", None),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

// ── get_context_window provider success ─────────────────────────────

#[tokio::test]
async fn context_window_resolves_limit_from_all_shape() {
    let mock = axum::Router::new().route(
        "/provider",
        get(|| async {
            axum::Json(json!({
                "all": [
                    { "models": { "small": { "limit": { "context": 128000 } } } },
                    { "models": { "big": { "limit": { "context": 500000 } } } }
                ]
            }))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/context-window?session_id=s1", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Max across models is used.
    assert_eq!(v["context_limit"], 500000);
    assert_eq!(v["total_used"], 0);
}

#[tokio::test]
async fn context_window_resolves_limit_from_flat_array_shape() {
    let mock = axum::Router::new().route(
        "/provider",
        get(|| async {
            axum::Json(json!([
                { "models": { "m": { "limit": { "context": 321000 } } } }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/context-window?session_id=s1", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["context_limit"], 321000);
}

#[tokio::test]
async fn context_window_provider_without_limits_falls_back() {
    // Reachable provider payload but no `/limit/context` anywhere → 200_000.
    let mock = axum::Router::new().route(
        "/provider",
        get(|| async { axum::Json(json!({ "all": [ { "models": {} } ] })) }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/context-window?session_id=s1", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["context_limit"], 200_000);
}
