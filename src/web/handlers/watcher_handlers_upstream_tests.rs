//! Mock-upstream tests for `watcher_handlers::get_watcher_messages`.
//!
//! Pre-existing tests cover the no-project (400) and upstream-down (500)
//! branches plus the pure `parse_watcher_messages` helper. Here a mock opencode
//! server drives the **success** path end-to-end: the `/session/{id}/message`
//! body is fetched, parsed, filtered to user messages, and reversed.

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
async fn get_watcher_messages_success_filters_and_reverses() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(json!([
                { "info": { "role": "user" }, "parts": [{ "type": "text", "text": "first" }] },
                { "info": { "role": "assistant" }, "parts": [{ "type": "text", "text": "ignore" }] },
                { "info": { "role": "user" }, "parts": [{ "type": "text", "text": "second" }] }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/watcher/s1/messages", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = v.as_array().unwrap();
    // Only the two user messages, most-recent first.
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["role"], "user");
    assert_eq!(arr[0]["text"], "second");
    assert_eq!(arr[1]["text"], "first");
}

#[tokio::test]
async fn get_watcher_messages_empty_when_no_user_messages() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(json!([
                { "info": { "role": "assistant" }, "parts": [{ "type": "text", "text": "hi" }] }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, body) = scope_base_url(
        base,
        send_json(router, "GET", "/api/watcher/s1/messages", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_watcher_messages_non_json_body_is_500() {
    // Reachable upstream but a non-JSON body → the `resp.json()` parse errors.
    let mock = axum::Router::new()
        .route("/session/{id}/message", get(|| async { "not json at all" }));
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let router = test_router(state);
    let (status, _) = scope_base_url(
        base,
        send_json(router, "GET", "/api/watcher/s1/messages", None),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
