//! Generated tests for `agents_handlers.rs`.
//!
//! The primary path proxies `GET {opencode}/agent`; with no server running it
//! fails fast and the handler falls back to reading project config files and
//! injecting the built-in `build`/`plan` defaults. We drive both the
//! no-config (defaults only) and config-present branches via the real router.

use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use axum::http::StatusCode;
use serde_json::json;

fn init_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

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

/// Seed a project whose directory is `tmp`, with the base-url unreachable.
async fn state_with_project_dir(tmp: &tempfile::TempDir) -> ServerState {
    isolate_env();
    init_base_url();
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), None)
        .await
        .expect("add project");
    state
}

fn body_json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

#[tokio::test]
async fn agents_no_project_bad_request() {
    init_base_url();
    let state = test_server_state();
    let (status, _) = send_json(test_router(state), "GET", "/api/agents", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn agents_defaults_when_no_config() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_with_project_dir(&tmp).await;
    let (status, body) = send_json(test_router(state), "GET", "/api/agents", None).await;
    assert_eq!(status, StatusCode::OK);
    let agents = body_json(&body);
    let arr = agents.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "build");
    assert_eq!(arr[0]["native"], true);
    assert_eq!(arr[1]["id"], "plan");
    assert_eq!(arr[1]["native"], true);
}

#[tokio::test]
async fn agents_from_config_custom_agent() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("opencode.json"),
        json!({
            "agents": {
                "custom": {
                    "description": "my agent",
                    "mode": "subagent",
                    "hidden": true,
                    "color": "red"
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    let state = state_with_project_dir(&tmp).await;
    let (status, body) = send_json(test_router(state), "GET", "/api/agents", None).await;
    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let arr = arr.as_array().unwrap();
    // build, plan, custom (sorted with build/plan first).
    assert_eq!(arr.len(), 3);
    let custom = arr.iter().find(|a| a["id"] == "custom").unwrap();
    assert_eq!(custom["label"], "Custom");
    assert_eq!(custom["description"], "my agent");
    assert_eq!(custom["mode"], "subagent");
    assert_eq!(custom["hidden"], true);
    assert_eq!(custom["color"], "red");
    assert_eq!(custom["native"], false);
}

#[tokio::test]
async fn agents_config_name_field_used_as_label() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("opencode.json"),
        json!({ "agents": { "x": { "name": "Fancy", "description": "d" } } }).to_string(),
    )
    .unwrap();

    let state = state_with_project_dir(&tmp).await;
    let (status, body) = send_json(test_router(state), "GET", "/api/agents", None).await;
    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let x = arr
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "x")
        .unwrap()
        .clone();
    assert_eq!(x["label"], "Fancy");
    assert_eq!(x["mode"], "all"); // default when unspecified
}

#[tokio::test]
async fn agents_config_defining_build_and_plan_not_duplicated() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("opencode.json"),
        json!({
            "agents": {
                "build": { "description": "custom build" },
                "plan": { "description": "custom plan" }
            }
        })
        .to_string(),
    )
    .unwrap();

    let state = state_with_project_dir(&tmp).await;
    let (status, body) = send_json(test_router(state), "GET", "/api/agents", None).await;
    assert_eq!(status, StatusCode::OK);
    let arr = body_json(&body);
    let arr = arr.as_array().unwrap();
    // No duplicate defaults inserted.
    assert_eq!(arr.len(), 2);
    let build = arr.iter().find(|a| a["id"] == "build").unwrap();
    assert_eq!(build["description"], "custom build");
    // native is forced true for build/plan even when config-defined.
    assert_eq!(build["native"], true);
}
