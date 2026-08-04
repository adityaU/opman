//! Mock-upstream tests for `agents_handlers::get_agents`.
//!
//! The pre-existing `agents_handlers_tests.rs` only exercises the *fallback*
//! (opencode unreachable → static-config + built-in defaults). These tests
//! stand up a mock opencode server so the **primary proxy path** runs: the
//! `GET {opencode}/agent` response is fetched, deserialised, and mapped into
//! `AgentEntry` values (capitalised label, hidden/native/color passthrough).

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

/// Seed a project (so `resolve_project_dir` succeeds) whose dir is a fresh temp
/// dir with no opencode config files.
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

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

#[tokio::test]
async fn upstream_agents_mapped_and_returned() {
    let mock = axum::Router::new().route(
        "/agent",
        get(|| async {
            axum::Json(json!([
                { "name": "build", "description": "Coding", "mode": "primary", "native": true },
                {
                    "name": "reviewer",
                    "description": "Reviews code",
                    "mode": "subagent",
                    "hidden": true,
                    "native": false,
                    "color": "blue"
                }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let (status, body) = scope_base_url(
        base,
        send_json(test_router(state), "GET", "/api/agents", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let arr = arr.as_array().unwrap();
    // Non-empty upstream returns directly — no defaults injected, no re-sort.
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "build");
    assert_eq!(arr[0]["label"], "Build"); // capitalised first letter
    assert_eq!(arr[1]["id"], "reviewer");
    assert_eq!(arr[1]["label"], "Reviewer");
    assert_eq!(arr[1]["description"], "Reviews code");
    assert_eq!(arr[1]["mode"], "subagent");
    assert_eq!(arr[1]["hidden"], true);
    assert_eq!(arr[1]["native"], false);
    assert_eq!(arr[1]["color"], "blue");
}

#[tokio::test]
async fn upstream_empty_name_yields_empty_label_and_defaults() {
    // Missing fields exercise every `.unwrap_or` default in the mapper and the
    // empty-name → `chars.next() == None` label branch.
    let mock = axum::Router::new().route(
        "/agent",
        get(|| async { axum::Json(json!([ { "name": "" } ])) }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let (status, body) = scope_base_url(
        base,
        send_json(test_router(state), "GET", "/api/agents", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let arr = arr.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "");
    assert_eq!(arr[0]["label"], "");
    assert_eq!(arr[0]["mode"], "all"); // default
    assert_eq!(arr[0]["hidden"], false);
    assert_eq!(arr[0]["native"], false);
    assert!(arr[0]["color"].is_null());
}

#[tokio::test]
async fn upstream_empty_array_falls_through_to_defaults() {
    // A successful but empty upstream list is treated as "no agents" and the
    // static-config fallback (built-in build/plan defaults) is used instead.
    let mock = axum::Router::new().route("/agent", get(|| async { axum::Json(json!([])) }));
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let (status, body) = scope_base_url(
        base,
        send_json(test_router(state), "GET", "/api/agents", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let arr = arr.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "build");
    assert_eq!(arr[1]["id"], "plan");
}

#[tokio::test]
async fn upstream_non_success_status_falls_through_to_defaults() {
    // Upstream reachable but returns 500 → not success → fallback defaults.
    let mock = axum::Router::new().route(
        "/agent",
        get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let (status, body) = scope_base_url(
        base,
        send_json(test_router(state), "GET", "/api/agents", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    assert_eq!(arr.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn upstream_non_array_body_falls_through_to_defaults() {
    // 200 but a non-array JSON body → `.json::<Vec<_>>()` errors → fallback.
    let mock = axum::Router::new().route(
        "/agent",
        get(|| async { axum::Json(json!({ "unexpected": "shape" })) }),
    );
    let base = start_mock_upstream(mock).await;
    let (state, _tmp) = state_with_project().await;

    let (status, body) = scope_base_url(
        base,
        send_json(test_router(state), "GET", "/api/agents", None),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    assert_eq!(arr.as_array().unwrap().len(), 2);
}
