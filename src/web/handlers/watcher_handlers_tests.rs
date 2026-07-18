//! Generated coverage tests for `watcher_handlers.rs`.

use super::*;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

fn auth() -> AuthUser {
    AuthUser { subject: "t".into() }
}

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

async fn status<T: IntoResponse>(r: Result<T, WebError>) -> axum::http::StatusCode {
    r.into_response().status()
}

fn req(session_id: &str, cont: &str, idle: u64) -> WatcherConfigRequest {
    WatcherConfigRequest {
        session_id: session_id.into(),
        project_idx: 0,
        idle_timeout_secs: idle,
        continuation_message: cont.into(),
        include_original: false,
        original_message: None,
        hang_message: "hang".into(),
        hang_timeout_secs: 180,
    }
}

// ── list / create / get / delete ───────────────────────────────────

#[tokio::test]
async fn list_watchers_empty() {
    let state = test_server_state();
    let resp = list_watchers(State(state), auth()).await.unwrap().into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn create_watcher_empty_session_400() {
    let state = test_server_state();
    let st = status(create_watcher(State(state), auth(), axum::Json(req("", "go", 10))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_watcher_empty_continuation_400() {
    let state = test_server_state();
    let st = status(create_watcher(State(state), auth(), axum::Json(req("s1", "   ", 10))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_watcher_zero_timeout_400() {
    let state = test_server_state();
    let st = status(create_watcher(State(state), auth(), axum::Json(req("s1", "go", 0))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_watcher_ok_then_get_and_delete() {
    let state = test_server_state();
    // create
    let st = status(create_watcher(State(state.clone()), auth(), axum::Json(req("s1", "continue", 30))).await).await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // list contains one
    let resp = list_watchers(State(state.clone()), auth()).await.unwrap().into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);

    // get existing
    let st = status(get_watcher(State(state.clone()), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // delete existing
    let st = status(delete_watcher(State(state.clone()), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::OK);

    // delete again → 404
    let st = status(delete_watcher(State(state.clone()), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_watcher_missing_404() {
    let state = test_server_state();
    let st = status(get_watcher(State(state), auth(), Path("ghost".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_watcher_sessions_empty_ok() {
    let state = test_server_state();
    let resp = get_watcher_sessions(State(state), auth()).await.unwrap().into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

// ── get_watcher_messages ───────────────────────────────────────────

#[tokio::test]
async fn get_watcher_messages_no_project_400() {
    let state = test_server_state();
    let st = status(get_watcher_messages(State(state), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_watcher_messages_upstream_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // upstream opencode server is unreachable → Internal error
    let st = status(get_watcher_messages(State(state), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}
