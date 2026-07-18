//! Generated coverage tests (supplement) for `files_handlers.rs` — remaining
//! reachable branches not hit by the helpers/browse/mutate/edge suites:
//! gitignore star-pattern non-glob arm, directory matches in sync search,
//! the search-handler `limit.min(50)` cap, absolute-path read traversal, and
//! a multi-file upload exercising the write loop twice.

use super::*;

use axum::extract::{Query, State};
use axum::response::IntoResponse;
use crate::web::auth::AuthUser;
use crate::web::test_support::{test_router, test_server_state};
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

// ── is_gitignored: '*' pattern that is NOT a "*.ext" glob ────────────

#[test]
fn is_gitignored_star_non_glob_no_match() {
    // Pattern contains '*' but does not start with "*." → strip_prefix("*.")
    // yields None → the inner glob check is skipped → no match on this arm.
    let pats = vec!["build*".to_string()];
    assert!(!is_gitignored("build/out.js", "out.js", false, &pats));
    // Another non-"*." glob shape.
    let pats2 = vec!["a*b".to_string()];
    assert!(!is_gitignored("some/file.txt", "file.txt", false, &pats2));
}

#[test]
fn is_gitignored_path_pattern_exact_equality() {
    // Pattern with '/' matched by exact equality (rel_path == pat), not prefix.
    let pats = vec!["docs/readme".to_string()];
    assert!(is_gitignored("docs/readme", "readme", false, &pats));
}

// ── search_files_sync: a directory entry can be a match result ───────

#[test]
fn search_files_sync_matches_directory_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("findable_dir")).unwrap();
    std::fs::write(tmp.path().join("findable_dir/inner.txt"), "").unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let results = search_files_sync(&root, "findable_dir", 10);
    // The directory itself is returned with is_dir = true.
    assert!(results.iter().any(|e| e.name == "findable_dir" && e.is_dir));
}

// ── search_files handler: limit above the hard cap of 50 ────────────

#[tokio::test]
async fn search_files_handler_caps_limit_at_50() {
    let tmp = tempfile::TempDir::new().unwrap();
    for i in 0..70 {
        std::fs::write(tmp.path().join(format!("hit_{i:03}.txt")), "").unwrap();
    }
    let state = state_dir(tmp.path());
    // limit 1000 → capped to 50 internally.
    let resp = search_files(
        State(state),
        auth(),
        Query(FileSearchQuery { q: "hit".into(), limit: 1000 }),
    )
    .await
    .unwrap()
    .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["entries"].as_array().unwrap().len(), 50);
}

// ── read_file / read_file_raw: absolute path escaping project → 400 ──

#[tokio::test]
async fn read_file_absolute_path_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    // A real file that exists but lives OUTSIDE the project directory.
    let outside = root.path().join("secret.txt");
    std::fs::write(&outside, "s").unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        read_file(
            State(state),
            auth(),
            Query(FileReadQuery { path: outside.to_string_lossy().to_string() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn read_file_raw_absolute_path_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let outside = root.path().join("secret.bin");
    std::fs::write(&outside, [0u8, 1, 2]).unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        read_file_raw(
            State(state),
            auth(),
            Query(FileReadQuery { path: outside.to_string_lossy().to_string() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── upload: two files in one request exercises the write loop twice ──

async fn send_multipart(router: axum::Router, uri: &str, body: Vec<u8>) -> axum::http::StatusCode {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "multipart/form-data; boundary=BOUND")
        .body(Body::from(body))
        .unwrap();
    router.oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn upload_multiple_files_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let router = test_router(state_dir(tmp.path()));
    let mut b = String::new();
    for (name, data) in [("one.txt", "1"), ("two.txt", "2")] {
        b.push_str("--BOUND\r\n");
        b.push_str(&format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\r\n"
        ));
        b.push_str(data);
        b.push_str("\r\n");
    }
    b.push_str("--BOUND--\r\n");
    let st = send_multipart(router, "/api/file/upload", b.into_bytes()).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(std::fs::read_to_string(tmp.path().join("one.txt")).unwrap(), "1");
    assert_eq!(std::fs::read_to_string(tmp.path().join("two.txt")).unwrap(), "2");
}
