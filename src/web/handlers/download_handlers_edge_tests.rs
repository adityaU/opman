//! Generated coverage tests (wave 2) for `download_handlers.rs`.
//!
//! Covers the branches the first test file missed: the `canonicalize()`
//! failure on a non-existent project base (→ Internal 500), an explicit "."
//! directory target, and zipping an empty directory (the `add_dir_recursive`
//! no-entries path).

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

async fn parts<T: IntoResponse>(
    r: Result<T, WebError>,
) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let resp = r.into_response();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

// ── canonicalize(base) failure → Internal 500 ──────────────────────

#[tokio::test]
async fn download_file_base_canonicalize_error_500() {
    // Project path points at a directory that does not exist on disk, so
    // `base.canonicalize()` fails before the target is ever resolved.
    let missing = std::path::Path::new("/nonexistent-opman-download-base-xyz");
    let state = state_dir(missing);
    let (st, _, _) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "f.txt".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn download_dir_base_canonicalize_error_500() {
    let missing = std::path::Path::new("/nonexistent-opman-download-base-abc");
    let state = state_dir(missing);
    let (st, _, _) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "sub".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

// ── explicit "." path resolves to project root ─────────────────────

#[tokio::test]
async fn download_dir_dot_path_zips_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
    let state = state_dir(tmp.path());
    let (st, headers, body) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: ".".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(headers.get(axum::http::header::CONTENT_TYPE).unwrap(), "application/zip");
    assert_eq!(&body[0..2], b"PK");
}

// ── empty directory → valid (empty) zip ────────────────────────────

#[tokio::test]
async fn download_dir_empty_dir_zip() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("empty")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _, body) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "empty".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    // Even an empty archive carries the ZIP end-of-central-directory record.
    assert_eq!(&body[0..2], b"PK");
}

// ── directory containing only hidden entries → all skipped ─────────

#[tokio::test]
async fn download_dir_only_hidden_entries_skipped() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join(".secret"), "s").unwrap();
    std::fs::create_dir(sub.join(".git")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _, body) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "sub".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(&body[0..2], b"PK");
}
