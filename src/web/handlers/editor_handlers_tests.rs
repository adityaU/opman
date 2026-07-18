//! Generated coverage tests for `editor_handlers.rs`.
//!
//! The LSP handlers proxy to a live Neovim socket. Without a registered socket
//! they fail fast with BadRequest; with a bogus socket path the RPC connection
//! fails and they return Internal. The success tail (parsing a live LSP reply)
//! requires a running Neovim + LSP and is not exercised here.

use super::*;

use axum::extract::{Query, State};
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

async fn seed_socket(state: &ServerState, session_id: &str, sock: std::path::PathBuf) {
    let mut reg = state.nvim_registry.write().await;
    reg.insert((0, session_id.to_string()), sock);
}

fn lsp_query() -> EditorLspQuery {
    EditorLspQuery {
        path: "a.rs".into(),
        session_id: "sess".into(),
        line: Some(1),
        col: Some(0),
    }
}

// ── no socket registered → BadRequest ──────────────────────────────

#[tokio::test]
async fn diagnostics_no_socket_400() {
    let state = test_server_state();
    let st = status(editor_lsp_diagnostics(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hover_no_socket_400() {
    let state = test_server_state();
    let st = status(editor_lsp_hover(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn definition_no_socket_400() {
    let state = test_server_state();
    let st = status(editor_lsp_definition(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn format_no_socket_400() {
    let state = test_server_state();
    let st = status(
        editor_lsp_format(
            State(state),
            auth(),
            axum::Json(EditorFormatRequest { path: "a.rs".into(), session_id: "sess".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── bogus socket → connection failure → Internal ───────────────────

#[tokio::test]
async fn diagnostics_bogus_socket_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", tmp.path().join("no.sock")).await;
    let st = status(editor_lsp_diagnostics(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn hover_bogus_socket_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", tmp.path().join("no.sock")).await;
    let st = status(editor_lsp_hover(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn definition_bogus_socket_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", tmp.path().join("no.sock")).await;
    let st = status(editor_lsp_definition(State(state), auth(), Query(lsp_query())).await).await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn format_bogus_socket_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    seed_socket(&state, "sess", tmp.path().join("no.sock")).await;
    let st = status(
        editor_lsp_format(
            State(state),
            auth(),
            axum::Json(EditorFormatRequest { path: "a.rs".into(), session_id: "sess".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}
