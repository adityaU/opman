//! Generated coverage tests for `download_handlers.rs`.

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

async fn parts<T: IntoResponse>(r: Result<T, WebError>) -> (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let resp = r.into_response();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

// ── download_file ──────────────────────────────────────────────────

#[tokio::test]
async fn download_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("doc.pdf"), b"%PDF-1.4").unwrap();
    let state = state_dir(tmp.path());
    let (st, headers, body) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "doc.pdf".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(headers.get(axum::http::header::CONTENT_TYPE).unwrap(), "application/pdf");
    let cd = headers.get(axum::http::header::CONTENT_DISPOSITION).unwrap().to_str().unwrap();
    assert!(cd.contains("attachment"));
    assert!(cd.contains("doc.pdf"));
    assert_eq!(body, b"%PDF-1.4");
}

#[tokio::test]
async fn download_file_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _, _) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "ghost".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_file_on_dir_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("d")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _, _) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "d".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn download_file_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(root.path().join("out.txt"), "x").unwrap();
    let state = state_dir(&proj);
    let (st, _, _) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "../out.txt".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn download_file_no_project_400() {
    let state = test_server_state();
    let (st, _, _) = parts(
        download_file(State(state), auth(), Query(FileDownloadQuery { path: "x".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── download_dir ───────────────────────────────────────────────────

#[tokio::test]
async fn download_dir_ok_zip() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("proj/nested")).unwrap();
    std::fs::write(tmp.path().join("proj/a.txt"), "aaa").unwrap();
    std::fs::write(tmp.path().join("proj/nested/b.txt"), "bbb").unwrap();
    std::fs::write(tmp.path().join("proj/.hidden"), "secret").unwrap();
    let state = state_dir(tmp.path());
    let (st, headers, body) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "proj".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(headers.get(axum::http::header::CONTENT_TYPE).unwrap(), "application/zip");
    // ZIP magic bytes "PK"
    assert_eq!(&body[0..2], b"PK");
    assert!(body.len() > 4);
}

#[tokio::test]
async fn download_dir_default_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "data").unwrap();
    let state = state_dir(tmp.path());
    let (st, _, body) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: String::new() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(&body[0..2], b"PK");
}

#[tokio::test]
async fn download_dir_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _, _) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "ghostdir".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_dir_on_file_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    let (st, _, _) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "f.txt".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn download_dir_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::create_dir(root.path().join("outdir")).unwrap();
    let state = state_dir(&proj);
    let (st, _, _) = parts(
        download_dir(State(state), auth(), Query(DirDownloadQuery { path: "../outdir".into() })).await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
