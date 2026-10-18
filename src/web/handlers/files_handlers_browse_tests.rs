//! Generated coverage tests for `files_handlers.rs` — browse/read/write/create handlers.

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
        .expect("test setup succeeds")
        .to_vec();
    (status, bytes)
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("test setup succeeds")
}

// ── browse_files ───────────────────────────────────────────────────

#[tokio::test]
async fn browse_root_lists_sorted_skips_hidden() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("zebra.txt"), "z").expect("test setup succeeds");
    std::fs::write(tmp.path().join("apple.txt"), "a").expect("test setup succeeds");
    std::fs::write(tmp.path().join(".hidden"), "h").expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("dir1")).expect("test setup succeeds");
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
    let v = json(&body);
    let entries = v["entries"].as_array().expect("test setup succeeds");
    // dir first, hidden excluded
    assert_eq!(entries[0]["name"], "dir1");
    assert!(entries[0]["is_dir"].as_bool().expect("test setup succeeds"));
    let names: Vec<_> = entries
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds"))
        .collect();
    assert!(!names.contains(&".hidden"));
    assert!(names.contains(&"apple.txt"));
}

#[tokio::test]
async fn browse_subdir_path() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("sub")).expect("test setup succeeds");
    std::fs::write(tmp.path().join("sub/inner.txt"), "x").expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery { path: "sub".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    assert_eq!(v["path"], "sub");
    assert_eq!(v["entries"][0]["path"], "sub/inner.txt");
}

#[tokio::test]
async fn browse_missing_dir_404() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery {
                path: "ghost".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn browse_traversal_400() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    let state = state_dir(&proj);
    let (st, _) = parts(
        browse_files(
            State(state),
            auth(),
            Query(FileBrowseQuery { path: "..".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_no_project_400() {
    let state = test_server_state();
    let (st, _) = parts(
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
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_skips_symlink_escaping_project() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    let outside = root.path().join("outside.txt");
    std::fs::write(&outside, "secret").expect("test setup succeeds");
    // symlink inside project pointing outside → should be skipped
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, proj.join("escape")).expect("test setup succeeds");
    std::fs::write(proj.join("normal.txt"), "ok").expect("test setup succeeds");
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
    let v = json(&body);
    let names: Vec<_> = v["entries"]
        .as_array()
        .expect("test setup succeeds")
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds").to_string())
        .collect();
    assert!(names.contains(&"normal.txt".to_string()));
    assert!(!names.contains(&"escape".to_string()));
}

#[tokio::test]
async fn browse_keeps_symlink_inside_project() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("target.txt"), "t").expect("test setup succeeds");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path().join("target.txt"), tmp.path().join("link.txt"))
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
    let v = json(&body);
    let names: Vec<_> = v["entries"]
        .as_array()
        .expect("test setup succeeds")
        .iter()
        .map(|e| e["name"].as_str().expect("test setup succeeds").to_string())
        .collect();
    assert!(names.contains(&"link.txt".to_string()));
}

#[path = "files_handlers_read_write_tests.rs"]
mod extra;
