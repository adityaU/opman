//! Generated coverage tests (wave 2) for `files_handlers.rs` — remaining
//! filesystem error/edge branches: bad-base 500s, symlink relative/dangling
//! variants, directory-read errors, rename dest-parent traversal + self-move,
//! upload overwrite / "." directory.

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

/// A project whose configured working dir does not exist on disk — forces the
/// `base.canonicalize()` error branches (→ Internal 500) in every handler.
fn state_bad_base() -> ServerState {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let missing = tmp.path().join("does_not_exist_dir");
    // TempDir dropped here — path is guaranteed gone.
    state_dir(&missing)
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
        .expect("test setup succeeds")
        .to_vec();
    (status, bytes)
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("test setup succeeds")
}

const ISE: axum::http::StatusCode = axum::http::StatusCode::INTERNAL_SERVER_ERROR;

// ── base.canonicalize() failure → Internal 500 (one per handler) ────

#[tokio::test]
async fn browse_bad_base_500() {
    let (st, _) = parts(
        browse_files(
            State(state_bad_base()),
            auth(),
            Query(FileBrowseQuery {
                path: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn write_bad_base_500() {
    let (st, _) = parts(
        write_file(
            State(state_bad_base()),
            auth(),
            axum::Json(FileWriteRequest {
                path: "f.txt".into(),
                content: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn create_file_bad_base_500() {
    let (st, _) = parts(
        create_file(
            State(state_bad_base()),
            auth(),
            axum::Json(FileCreateRequest {
                path: "f.txt".into(),
                content: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn create_dir_bad_base_500() {
    let (st, _) = parts(
        create_dir(
            State(state_bad_base()),
            auth(),
            axum::Json(DirCreateRequest { path: "d".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn delete_file_bad_base_500() {
    let (st, _) = parts(
        delete_file(
            State(state_bad_base()),
            auth(),
            axum::Json(FileDeleteRequest { path: "f".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn delete_dir_bad_base_500() {
    let (st, _) = parts(
        delete_dir(
            State(state_bad_base()),
            auth(),
            axum::Json(DirDeleteRequest { path: "d".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn rename_bad_base_500() {
    let (st, _) = parts(
        rename_entry(
            State(state_bad_base()),
            auth(),
            axum::Json(RenameRequest {
                from_path: "a".into(),
                to_path: "b".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

// ── browse symlink variants ─────────────────────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn browse_relative_symlink_inside_kept() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("target.txt"), "t").expect("test setup succeeds");
    // relative symlink pointing at a sibling inside the project → kept
    std::os::unix::fs::symlink("target.txt", tmp.path().join("rel_link"))
        .expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery {
                path: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"]
        .as_array()
        .expect("test setup succeeds")
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds").to_string())
        .collect();
    assert!(names.contains(&"rel_link".to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn browse_dangling_symlink_skipped() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    // symlink to a non-existent target → read_link Ok but canonicalize Err → skipped
    std::os::unix::fs::symlink("nonexistent_target", tmp.path().join("dangling"))
        .expect("test setup succeeds");
    std::fs::write(tmp.path().join("real.txt"), "r").expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery {
                path: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"]
        .as_array()
        .expect("test setup succeeds")
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds").to_string())
        .collect();
    assert!(names.contains(&"real.txt".to_string()));
    assert!(!names.contains(&"dangling".to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn browse_relative_symlink_escaping_skipped() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    std::fs::write(root.path().join("outside.txt"), "s").expect("test setup succeeds");
    // relative symlink that escapes the project via ".."
    std::os::unix::fs::symlink("../outside.txt", proj.join("escape_rel"))
        .expect("test setup succeeds");
    std::fs::write(proj.join("keep.txt"), "k").expect("test setup succeeds");
    let state = state_dir(&proj);
    let (st, body) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery {
                path: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"]
        .as_array()
        .expect("test setup succeeds")
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds").to_string())
        .collect();
    assert!(names.contains(&"keep.txt".to_string()));
    assert!(!names.contains(&"escape_rel".to_string()));
}

#[path = "files_handlers_edge_extra_tests.rs"]
mod extra;
