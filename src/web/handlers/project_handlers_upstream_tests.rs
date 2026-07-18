//! Mock-upstream tests for `project_handlers::new_session`.
//!
//! The pre-existing tests only cover the invalid-index and upstream-down
//! branches. Here a mock opencode server lets the **success** path run: the
//! `POST {opencode}/session` response is parsed into `SessionInfo`, the session
//! is registered + activated, and a `NewSessionResponse` is returned. We also
//! cover the non-success-status and unparseable-session-info error branches.

use crate::web::test_support::{
    scope_base_url, send_json, start_mock_upstream, test_router, test_server_state,
};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use axum::routing::post;
use serde_json::json;

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

#[tokio::test]
async fn new_session_success_registers_and_returns_id() {
    let mock = axum::Router::new().route(
        "/session",
        post(|| async {
            axum::Json(json!({ "id": "sess-x", "title": "New", "directory": "/tmp/x" }))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state.clone());
    let (status, body) = scope_base_url(
        base,
        send_json(router, "POST", "/api/session/new", Some(json!({ "project_idx": 0 }))),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["session_id"], "sess-x");

    // The new session was added + activated in web_state.
    let active = state.web_state.active_session_id().await;
    assert_eq!(active.as_deref(), Some("sess-x"));
}

#[tokio::test]
async fn new_session_non_success_status_is_500() {
    // Upstream returns a JSON body but a 500 status → handler surfaces Internal.
    let mock = axum::Router::new().route(
        "/session",
        post(|| async {
            (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({ "error": "nope" })))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "POST", "/api/session/new", Some(json!({ "project_idx": 0 }))),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn new_session_unparseable_info_is_500() {
    // 200 but the body has no "id" → `from_value::<SessionInfo>` fails.
    let mock = axum::Router::new()
        .route("/session", post(|| async { axum::Json(json!({ "title": "x" })) }));
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "POST", "/api/session/new", Some(json!({ "project_idx": 0 }))),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn new_session_non_json_body_is_500() {
    // Body isn't JSON → the `resp.json()` parse fails → Internal.
    let mock = axum::Router::new()
        .route("/session", post(|| async { "plain text" }));
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "POST", "/api/session/new", Some(json!({ "project_idx": 0 }))),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
