//! Coverage tests (wave 3) for `files_handlers.rs` — remaining local-fs
//! branches not hit by earlier suites: `browse_files` reading a path that
//! canonicalizes to a *file* (read_dir fails → Internal 500), `search_files`
//! resolving no project (BadRequest), and `search_files` returning results
//! when a nested directory matches (is_dir path through the async handler).

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use axum::extract::{Query, State};
use axum::response::IntoResponse;

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

async fn parts<T: IntoResponse>(r: Result<T, WebError>) -> (axum::http::StatusCode, Vec<u8>) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

// ── browse_files on a FILE path → read_dir fails → Internal 500 ─────

#[tokio::test]
async fn browse_on_file_path_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("plain.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    // The path canonicalizes fine and is inside the project, but it's a file,
    // so `tokio::fs::read_dir` errors → Internal 500.
    let (st, _) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery {
                path: "plain.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

// ── search_files with no active project → BadRequest ────────────────

#[tokio::test]
async fn search_files_no_project_400() {
    let state = test_server_state();
    let (st, _) = parts(
        search_files(
            State(state),
            auth(),
            Query(FileSearchQuery {
                q: "x".into(),
                limit: 10,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── search_files async handler returns a directory match with is_dir=true ─

#[tokio::test]
async fn search_files_handler_returns_dir_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("uniquedirname")).unwrap();
    std::fs::write(tmp.path().join("uniquedirname/inner.txt"), "").unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        search_files(
            State(state),
            auth(),
            Query(FileSearchQuery {
                q: "uniquedirname".into(),
                limit: 20,
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    let entries = v["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .any(|e| e["name"] == "uniquedirname" && e["is_dir"] == true));
}
