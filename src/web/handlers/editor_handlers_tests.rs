//! Tests for the editor LSP handlers.
//!
//! The handlers no longer need a Neovim socket — they resolve a language server
//! themselves. What matters here is that the *benign* cases answer
//! `available: false` with a well-formed body instead of erroring, because the
//! editor renders availability and a 500 would turn "this is a .txt file" into
//! a failure toast. Cases that reach a real language server are covered
//! end-to-end, not here.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::State;
use axum::response::IntoResponse;

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

fn state_dir(dir: &std::path::Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), dir.to_path_buf())]);
    state
}

fn query(path: &str) -> EditorLspQuery {
    EditorLspQuery {
        path: path.into(),
        session_id: "s".into(),
        line: Some(1),
        col: Some(1),
        content: None,
        trigger: None,
    }
}

async fn body_of<T: IntoResponse>(response: T) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_response().into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

/// A file type with no language server must not be an error.
#[tokio::test]
async fn diagnostics_unavailable_for_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.unknownext"), "hello").unwrap();
    let state = state_dir(dir.path());

    let response =
        editor_lsp_diagnostics(State(state), auth(), Json(query("notes.unknownext"))).await;
    let value = body_of(response.expect("handler should not error")).await;

    assert_eq!(value["available"], false);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

#[tokio::test]
async fn hover_unavailable_for_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.unknownext"), "hello").unwrap();
    let state = state_dir(dir.path());

    let response = editor_lsp_hover(State(state), auth(), Json(query("notes.unknownext"))).await;
    let value = body_of(response.expect("handler should not error")).await;

    assert_eq!(value["available"], false);
    assert!(value["hover"].is_null());
}

#[tokio::test]
async fn definition_unavailable_for_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.unknownext"), "hello").unwrap();
    let state = state_dir(dir.path());

    let response =
        editor_lsp_definition(State(state), auth(), Json(query("notes.unknownext"))).await;
    let value = body_of(response.expect("handler should not error")).await;

    assert_eq!(value["available"], false);
    assert_eq!(value["locations"], serde_json::json!([]));
}

/// Format must hand back the original text untouched when it cannot run, so the
/// editor never replaces a buffer with nothing.
#[tokio::test]
async fn format_returns_original_when_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.unknownext"), "keep me").unwrap();
    let state = state_dir(dir.path());

    let response = editor_lsp_format(
        State(state),
        auth(),
        Json(EditorFormatRequest {
            path: "notes.unknownext".into(),
            session_id: "s".into(),
            content: None,
        }),
    )
    .await;
    let value = body_of(response.expect("handler should not error")).await;

    assert_eq!(value["available"], false);
    assert_eq!(value["formatted"], false);
    assert_eq!(value["content"], "keep me");
}

/// The project sandbox still applies — this is a real client error, not a
/// missing capability.
#[tokio::test]
async fn path_outside_project_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_dir(dir.path());

    let response = editor_lsp_hover(State(state), auth(), Json(query("../../etc/passwd"))).await;
    assert!(response.is_err(), "traversal must not resolve");
}

/// Completion degrades the same way as everything else.
#[tokio::test]
async fn completion_unavailable_for_unknown_extension() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.unknownext"), "hello").unwrap();
    let state = state_dir(dir.path());

    let response =
        editor_lsp_completion(State(state), auth(), Json(query("notes.unknownext"))).await;
    let value = body_of(response.expect("handler should not error")).await;

    assert_eq!(value["available"], false);
    assert_eq!(value["items"], serde_json::json!([]));
}

/// A missing file is a 404 rather than a silent empty answer.
#[tokio::test]
async fn missing_file_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_dir(dir.path());

    let response = editor_lsp_diagnostics(State(state), auth(), Json(query("nope.rs"))).await;
    assert!(response.is_err(), "missing file must not resolve");
}
