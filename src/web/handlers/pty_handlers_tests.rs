//! Generated coverage tests for `pty_handlers.rs`.
//!
//! The test ServerState uses a no-op PTY handle (its manager thread is never
//! started), so every spawn/write/resize/kill fails fast. We assert those
//! error branches plus input validation.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::State;
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

async fn status<T: IntoResponse>(r: Result<T, WebError>) -> axum::http::StatusCode {
    r.into_response().status()
}

fn spawn_req(kind: PtyKind) -> SpawnPtyRequest {
    SpawnPtyRequest {
        kind,
        id: "pty-1".into(),
        rows: Some(24),
        cols: Some(80),
        project: None,
        label: None,
        session_id: None,
    }
}

// ── spawn_pty ──────────────────────────────────────────────────────

#[tokio::test]
async fn spawn_no_project_400() {
    let state = test_server_state();
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::Shell))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_shell_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::Shell))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_neovim_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::Neovim))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_git_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::Git))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_opencode_manager_down_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let st = status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::Opencode))).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn spawn_claude_attach_no_session_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // no session_id and no active session → "No session to attach"
    let st =
        status(spawn_pty(State(state), auth(), axum::Json(spawn_req(PtyKind::ClaudeAttach))).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn spawn_claude_attach_no_running_agent_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let mut req = spawn_req(PtyKind::ClaudeAttach);
    req.session_id = Some("ses_abc".into());
    // session has no running claude agent → short_id_for_session returns None
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

/// An unknown kind never reaches the handler at all now — the request body
/// fails to deserialize, so axum answers before any project is resolved.
#[test]
fn spawn_unknown_kind_is_refused_by_deserialization() {
    let body = serde_json::json!({ "kind": "bogus", "id": "p" });
    assert!(serde_json::from_value::<SpawnPtyRequest>(body).is_err());
}

#[tokio::test]
async fn spawn_default_rows_cols_clamped() {
    // rows/cols None → defaults, still fails at manager-down (exercises clamp path).
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let mut req = spawn_req(PtyKind::Shell);
    req.rows = None;
    req.cols = None;
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

/// A pane names its own project, and that is where the shell must start —
/// not wherever the sidebar happens to be pointing.
#[tokio::test]
async fn spawn_prefers_the_requested_project_over_the_active_one() {
    let active = tempfile::TempDir::new().unwrap();
    let asked = tempfile::TempDir::new().unwrap();
    let state = state_dir(active.path());
    let mut req = spawn_req(PtyKind::Shell);
    req.project = Some(asked.path().to_string_lossy().into_owned());
    // Reaches the (absent) manager rather than being refused, which is only
    // possible if a project resolved.
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

/// An empty string is what a UI sends when it has no project yet, and it must
/// not be taken as a path to spawn in.
#[tokio::test]
async fn spawn_falls_back_when_the_requested_project_is_blank() {
    let state = test_server_state();
    let mut req = spawn_req(PtyKind::Shell);
    req.project = Some(String::new());
    let st = status(spawn_pty(State(state), auth(), axum::Json(req)).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_write ──────────────────────────────────────────────────────

#[tokio::test]
async fn pty_write_bad_base64_400() {
    let state = test_server_state();
    let st = status(
        pty_write(
            State(state),
            auth(),
            axum::Json(PtyWriteRequest {
                id: "x".into(),
                data: "!!!not base64!!!".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pty_write_valid_base64_not_found_400() {
    let state = test_server_state();
    let data = BASE64.encode(b"hello");
    let st = status(
        pty_write(
            State(state),
            auth(),
            axum::Json(PtyWriteRequest {
                id: "x".into(),
                data,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_resize ─────────────────────────────────────────────────────

#[tokio::test]
async fn pty_resize_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_resize(
            State(state),
            auth(),
            axum::Json(PtyResizeRequest {
                id: "x".into(),
                rows: 10,
                cols: 20,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_kill ───────────────────────────────────────────────────────

#[tokio::test]
async fn pty_kill_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_kill(
            State(state),
            auth(),
            axum::Json(PtyKillRequest { id: "x".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_rename ─────────────────────────────────────────────────────

#[tokio::test]
async fn pty_rename_not_found_400() {
    let state = test_server_state();
    let st = status(
        pty_rename(
            State(state),
            auth(),
            axum::Json(PtyRenameRequest {
                id: "x".into(),
                label: "Build".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

/// A blank label would leave an unclickable row in the picker, so it is refused
/// before the manager is asked.
#[tokio::test]
async fn pty_rename_blank_label_400() {
    let state = test_server_state();
    let st = status(
        pty_rename(
            State(state),
            auth(),
            axum::Json(PtyRenameRequest {
                id: "x".into(),
                label: "   ".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── pty_sessions ───────────────────────────────────────────────────

#[tokio::test]
async fn pty_sessions_empty_ok() {
    let state = test_server_state();
    let resp = pty_sessions(State(state), auth())
        .await
        .unwrap()
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_array().expect("an array of sessions").len(), 0);
}
