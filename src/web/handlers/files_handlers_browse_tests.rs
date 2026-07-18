//! Generated coverage tests for `files_handlers.rs` — browse/read/write/create handlers.

use super::*;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

fn auth() -> AuthUser {
    AuthUser { subject: "t".into() }
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

// ── browse_files ───────────────────────────────────────────────────

#[tokio::test]
async fn browse_root_lists_sorted_skips_hidden() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("zebra.txt"), "z").unwrap();
    std::fs::write(tmp.path().join("apple.txt"), "a").unwrap();
    std::fs::write(tmp.path().join(".hidden"), "h").unwrap();
    std::fs::create_dir(tmp.path().join("dir1")).unwrap();
    let state = state_dir(tmp.path());

    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    let entries = v["entries"].as_array().unwrap();
    // dir first, hidden excluded
    assert_eq!(entries[0]["name"], "dir1");
    assert!(entries[0]["is_dir"].as_bool().unwrap());
    let names: Vec<_> = entries.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert!(!names.contains(&".hidden"));
    assert!(names.contains(&"apple.txt"));
}

#[tokio::test]
async fn browse_subdir_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub/inner.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: "sub".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    assert_eq!(v["path"], "sub");
    assert_eq!(v["entries"][0]["path"], "sub/inner.txt");
}

#[tokio::test]
async fn browse_missing_dir_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: "ghost".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn browse_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: "..".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_no_project_400() {
    let state = test_server_state();
    let (st, _) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browse_skips_symlink_escaping_project() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let outside = root.path().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    // symlink inside project pointing outside → should be skipped
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, proj.join("escape")).unwrap();
    std::fs::write(proj.join("normal.txt"), "ok").unwrap();
    let state = state_dir(&proj);
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    let names: Vec<_> = v["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"normal.txt".to_string()));
    assert!(!names.contains(&"escape".to_string()));
}

#[tokio::test]
async fn browse_keeps_symlink_inside_project() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("target.txt"), "t").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path().join("target.txt"), tmp.path().join("link.txt")).unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    let names: Vec<_> = v["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"link.txt".to_string()));
}

// ── read_file / read_file_raw ──────────────────────────────────────

#[tokio::test]
async fn read_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        read_file(State(state), auth(), Query(FileReadQuery { path: "main.rs".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    assert_eq!(v["content"], "fn main() {}");
    assert_eq!(v["language"], "rust");
}

#[tokio::test]
async fn read_file_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file(State(state), auth(), Query(FileReadQuery { path: "ghost.rs".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn read_file_raw_ok_content_type() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("img.png"), [1u8, 2, 3]).unwrap();
    let state = state_dir(tmp.path());
    let resp = read_file_raw(State(state), auth(), Query(FileReadQuery { path: "img.png".into() }))
        .await
        .unwrap()
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(
        resp.headers().get(axum::http::header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn read_file_raw_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file_raw(State(state), auth(), Query(FileReadQuery { path: "no.png".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

// ── write_file ─────────────────────────────────────────────────────

#[tokio::test]
async fn write_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest { path: "new.txt".into(), content: "hello".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(std::fs::read_to_string(tmp.path().join("new.txt")).unwrap(), "hello");
}

#[tokio::test]
async fn write_file_missing_parent_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest { path: "no/such/dir/f.txt".into(), content: "x".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn write_file_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest { path: "../evil.txt".into(), content: "x".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── create_file ────────────────────────────────────────────────────

#[tokio::test]
async fn create_file_ok_201() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest { path: "fresh.txt".into(), content: "c".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED);
    assert!(tmp.path().join("fresh.txt").exists());
}

#[tokio::test]
async fn create_file_already_exists_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("dup.txt"), "old").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest { path: "dup.txt".into(), content: "new".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_file_missing_parent_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest { path: "ghostdir/f.txt".into(), content: "x".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

// ── create_dir ─────────────────────────────────────────────────────

#[tokio::test]
async fn create_dir_ok_nested_201() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_dir(State(state), auth(), axum::Json(DirCreateRequest { path: "a/b/c".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED);
    assert!(tmp.path().join("a/b/c").is_dir());
}

#[tokio::test]
async fn create_dir_already_exists_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("exists")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_dir(State(state), auth(), axum::Json(DirCreateRequest { path: "exists".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_dir_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        create_dir(State(state), auth(), axum::Json(DirCreateRequest { path: "../sneaky".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
