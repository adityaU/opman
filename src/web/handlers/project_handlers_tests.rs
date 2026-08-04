//! Generated tests for `project_handlers.rs`.
//!
//! Local endpoints assert real success/failure. `add_project`/`remove_project`
//! persist to a config file, so `XDG_CONFIG_HOME` is redirected to a temp dir.
//! `select_session`/`new_session` reach the opencode server; the base-url is
//! pointed at an unreachable port so they fail fast.

use super::*;
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

async fn add_temp_project(state: &ServerState) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), None)
        .await
        .expect("add project");
    tmp
}

// ── switch_project ──────────────────────────────────────────────────

#[tokio::test]
async fn switch_project_no_projects_bad_request() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/switch",
        Some(json!({ "index": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn switch_project_ok() {
    isolate_env();
    let state = test_server_state();
    let _tmp = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/switch",
        Some(json!({ "index": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn switch_project_invalid_index() {
    isolate_env();
    let state = test_server_state();
    let _tmp = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/switch",
        Some(json!({ "index": 99 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── select_session ──────────────────────────────────────────────────

#[tokio::test]
async fn select_session_invalid_project() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/select",
        Some(json!({ "project_idx": 0, "session_id": "s1" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn select_session_ok() {
    isolate_env();
    init_base_url();
    let state = test_server_state();
    let _tmp = add_temp_project(&state).await;
    let session: crate::app::SessionInfo =
        serde_json::from_value(json!({ "id": "sess1" })).unwrap();
    state.web_state.add_and_activate_session(0, session).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/select",
        Some(json!({ "project_idx": 0, "session_id": "sess1" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn select_session_unknown_session() {
    isolate_env();
    let state = test_server_state();
    let _tmp = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/select",
        Some(json!({ "project_idx": 0, "session_id": "nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── new_session ─────────────────────────────────────────────────────

#[tokio::test]
async fn new_session_invalid_project_index() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/new",
        Some(json!({ "project_idx": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn new_session_upstream_error() {
    isolate_env();
    init_base_url();
    let state = test_server_state();
    let _tmp = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/session/new",
        Some(json!({ "project_idx": 0 })),
    )
    .await;
    assert!(status.is_server_error(), "got {status}");
}

// ── add_project ─────────────────────────────────────────────────────

#[tokio::test]
async fn add_project_ok() {
    isolate_env();
    let state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/project/add",
        Some(json!({ "path": tmp.path().to_str().unwrap(), "name": "myproj" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["index"], 0);
    assert_eq!(v["name"], "myproj");
}

#[tokio::test]
async fn add_project_duplicate_bad_request() {
    isolate_env();
    let state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().to_str().unwrap().to_string();
    let router = test_router(state.clone());
    let (s1, _) = send_json(
        router,
        "POST",
        "/api/project/add",
        Some(json!({ "path": path })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    let (s2, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/add",
        Some(json!({ "path": path })),
    )
    .await;
    assert_eq!(s2, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_project_invalid_path_bad_request() {
    isolate_env();
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/add",
        Some(json!({ "path": "/definitely/not/a/real/path/xyz123" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── remove_project ──────────────────────────────────────────────────

#[tokio::test]
async fn remove_project_ok() {
    isolate_env();
    let state = test_server_state();
    let _t1 = add_temp_project(&state).await;
    let _t2 = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/remove",
        Some(json!({ "index": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn remove_project_last_bad_request() {
    isolate_env();
    let state = test_server_state();
    let _t1 = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/remove",
        Some(json!({ "index": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn remove_project_invalid_index_bad_request() {
    isolate_env();
    let state = test_server_state();
    let _t1 = add_temp_project(&state).await;
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/project/remove",
        Some(json!({ "index": 42 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── home_dir ────────────────────────────────────────────────────────

#[tokio::test]
async fn home_dir_ok() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/dirs/home", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["path"].is_string());
}

// ── browse_dirs ─────────────────────────────────────────────────────

#[tokio::test]
async fn browse_dirs_lists_visible_subdirs() {
    let state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("visible")).unwrap();
    std::fs::create_dir(tmp.path().join(".hidden")).unwrap();
    std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
    std::fs::write(tmp.path().join("afile.txt"), b"x").unwrap();

    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/dirs/browse",
        Some(json!({ "path": tmp.path().to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let names: Vec<&str> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["visible"]);
    assert_eq!(v["entries"][0]["is_project"], false);
}

#[tokio::test]
async fn browse_dirs_marks_existing_project() {
    isolate_env();
    let state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    let child = tmp.path().join("proj");
    std::fs::create_dir(&child).unwrap();
    state
        .web_state
        .add_project(child.to_str().unwrap(), None)
        .await
        .unwrap();

    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/dirs/browse",
        Some(json!({ "path": tmp.path().to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entry = &v["entries"][0];
    assert_eq!(entry["name"], "proj");
    assert_eq!(entry["is_project"], true);
}

#[tokio::test]
async fn browse_dirs_invalid_path_bad_request() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/dirs/browse",
        Some(json!({ "path": "/no/such/dir/xyz987" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_dirs_file_is_bad_request() {
    let state = test_server_state();
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("f.txt");
    std::fs::write(&file, b"x").unwrap();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/dirs/browse",
        Some(json!({ "path": file.to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_dirs_empty_path_uses_home() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/dirs/browse",
        Some(json!({ "path": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// ── toggle_panel / focus_panel ──────────────────────────────────────

#[tokio::test]
async fn toggle_panel_ok() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/panel/toggle",
        Some(json!({ "panel": "sidebar" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn toggle_panel_unknown_bad_request() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/panel/toggle",
        Some(json!({ "panel": "bogus" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn focus_panel_ok() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/panel/focus",
        Some(json!({ "panel": "GitPanel" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn focus_panel_unknown_bad_request() {
    let state = test_server_state();
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/panel/focus",
        Some(json!({ "panel": "bogus" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
