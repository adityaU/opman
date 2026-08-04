//! Coverage tests (wave 3) for `download_handlers.rs` — verify the zip archive
//! produced by `build_zip`/`add_dir_recursive` actually contains the expected
//! entries (nested directory + file arms, hidden-entry skip) by reading the
//! resulting archive back with `zip::ZipArchive`.

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

async fn body<T: IntoResponse>(r: Result<T, WebError>) -> (axum::http::StatusCode, Vec<u8>) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

/// Collect all entry names from a zip archive buffer.
fn zip_names(buf: &[u8]) -> Vec<String> {
    let reader = std::io::Cursor::new(buf.to_vec());
    let mut archive = zip::ZipArchive::new(reader).expect("valid zip");
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let f = archive.by_index(i).unwrap();
        names.push(f.name().to_string());
    }
    names
}

// ── nested dir + file entries present; hidden skipped ──────────────

#[tokio::test]
async fn download_dir_zip_roundtrip_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("bundle");
    std::fs::create_dir_all(root.join("level1/level2")).unwrap();
    std::fs::write(root.join("top.txt"), "top").unwrap();
    std::fs::write(root.join("level1/mid.txt"), "mid").unwrap();
    std::fs::write(root.join("level1/level2/deep.txt"), "deep").unwrap();
    std::fs::write(root.join(".hidden.txt"), "nope").unwrap();

    let state = state_dir(tmp.path());
    let (st, buf) = body(
        download_dir(
            State(state),
            auth(),
            Query(DirDownloadQuery {
                path: "bundle".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let names = zip_names(&buf);
    // Directory entries carry a trailing slash; file entries the relative path.
    assert!(names.iter().any(|n| n == "level1/"));
    assert!(names.iter().any(|n| n == "level1/level2/"));
    assert!(names.iter().any(|n| n == "top.txt"));
    assert!(names.iter().any(|n| n == "level1/mid.txt"));
    assert!(names.iter().any(|n| n == "level1/level2/deep.txt"));
    // Hidden file must be excluded.
    assert!(!names.iter().any(|n| n.contains(".hidden")));
}

// ── file content is preserved through the deflate round-trip ───────

#[tokio::test]
async fn download_dir_zip_file_content_preserved() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().join("data");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("payload.bin"), b"hello-zip-payload").unwrap();

    let state = state_dir(tmp.path());
    let (st, buf) = body(
        download_dir(
            State(state),
            auth(),
            Query(DirDownloadQuery {
                path: "data".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);

    let reader = std::io::Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    let mut f = archive.by_name("payload.bin").expect("entry present");
    let mut contents = Vec::new();
    std::io::Read::read_to_end(&mut f, &mut contents).unwrap();
    assert_eq!(contents, b"hello-zip-payload");
}
