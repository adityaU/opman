//! Generated coverage tests for `pty_handlers.rs`.
//!
//! The test ServerState uses a no-op PTY handle (its manager thread is never
//! started), so every spawn/write/resize/kill fails fast. We assert those
//! error branches plus input validation.

use super::*;

use axum::extract::State;
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
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

fn spawn_req(kind: &str) -> SpawnPtyRequest {
    SpawnPtyRequest {
        kind: kind.into(),
        id: "pty-1".into(),
        rows: Some(24),
        cols: Some(80),
        session_id: None,
    }
}

// ── spawn_pty ──────────────────────────────────────────────────────

#[tokio::test]
async fn spawn_no_project_400() {
    let state = test_server_state();
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("shell"))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_shell_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("shell"))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_neovim_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("neovim"))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_git_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("git"))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_opencode_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("opencode"))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_claude_attach_no_session_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // no session_id and no active session → "No session to attach"
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("claude-attach"))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_claude_attach_no_running_agent_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let mut req = spawn_req("claude-attach");
    req.session_id = Some("ses_abc".into());
    // session has no running claude agent → short_id_for_session returns None
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_unknown_kind_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req("bogus"))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_default_rows_cols_clamped() {
    // rows/cols None → defaults, still fails at manager-down (exercises clamp path).
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let req = SpawnPtyRequest {
        kind: "shell".into(),
        id: "p".into(),
        rows: None,
        cols: None,
        session_id: None,
    };
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

// ── pty_write ──────────────────────────────────────────────────────

#[tokio::test]
async fn pty_write_bad_base64_400() {
    let state = test_server_state();
    let st = status(
        pty_write(State(state), auth(), axum::Json(PtyWriteRequest { id: "x".into(), data: "!!!not base64!!!".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pty_write_valid_base64_not_found_400() {
    let state = test_server_state();
    let data = BASE64.encode(b"hello");
    let st = status(
        pty_write(State(state), auth(), axum::Json(PtyWriteRequest { id: "x".into(), data })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_resize ─────────────────────────────────────────────────────

#[tokio::test]
async fn pty_resize_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_resize(State(state), auth(), axum::Json(PtyResizeRequest { id: "x".into(), rows: 10, cols: 20 })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_kill ───────────────────────────────────────────────────────

#[tokio::test]
async fn pty_kill_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_kill(State(state), auth(), axum::Json(PtyKillRequest { id: "x".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_list ───────────────────────────────────────────────────────

#[tokio::test]
async fn pty_list_empty_ok() {
    let state = test_server_state();
    let resp = pty_list(State(state), auth()).await.unwrap().into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}
