//! Generated coverage tests (wave 2) for `files_handlers.rs` — remaining
//! filesystem error/edge branches: bad-base 500s, symlink relative/dangling
//! variants, directory-read errors, rename dest-parent traversal + self-move,
//! upload overwrite / "." directory.

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

/// A project whose configured working dir does not exist on disk — forces the
/// `base.canonicalize()` error branches (→ Internal 500) in every handler.
fn state_bad_base() -> ServerState {
    let tmp = tempfile::TempDir::new().unwrap();
    let missing = tmp.path().join("does_not_exist_dir");
    // TempDir dropped here — path is guaranteed gone.
    state_dir(&missing)
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

const ISE: axum::http::StatusCode = axum::http::StatusCode::INTERNAL_SERVER_ERROR;

// ── base.canonicalize() failure → Internal 500 (one per handler) ────

#[tokio::test]
async fn browse_bad_base_500() {
    let (st, _) = parts(
        browse_files(State(state_bad_base()), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn write_bad_base_500() {
    let (st, _) = parts(
        write_file(State(state_bad_base()), auth(),
            axum::Json(FileWriteRequest { path: "f.txt".into(), content: "x".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn create_file_bad_base_500() {
    let (st, _) = parts(
        create_file(State(state_bad_base()), auth(),
            axum::Json(FileCreateRequest { path: "f.txt".into(), content: "x".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn create_dir_bad_base_500() {
    let (st, _) = parts(
        create_dir(State(state_bad_base()), auth(),
            axum::Json(DirCreateRequest { path: "d".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn delete_file_bad_base_500() {
    let (st, _) = parts(
        delete_file(State(state_bad_base()), auth(),
            axum::Json(FileDeleteRequest { path: "f".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn delete_dir_bad_base_500() {
    let (st, _) = parts(
        delete_dir(State(state_bad_base()), auth(),
            axum::Json(DirDeleteRequest { path: "d".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn rename_bad_base_500() {
    let (st, _) = parts(
        rename_entry(State(state_bad_base()), auth(),
            axum::Json(RenameRequest { from_path: "a".into(), to_path: "b".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

// ── browse symlink variants ─────────────────────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn browse_relative_symlink_inside_kept() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("target.txt"), "t").unwrap();
    // relative symlink pointing at a sibling inside the project → kept
    std::os::unix::fs::symlink("target.txt", tmp.path().join("rel_link")).unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    ).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"].as_array().unwrap().iter()
        .map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"rel_link".to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn browse_dangling_symlink_skipped() {
    let tmp = tempfile::TempDir::new().unwrap();
    // symlink to a non-existent target → read_link Ok but canonicalize Err → skipped
    std::os::unix::fs::symlink("nonexistent_target", tmp.path().join("dangling")).unwrap();
    std::fs::write(tmp.path().join("real.txt"), "r").unwrap();
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    ).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"].as_array().unwrap().iter()
        .map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"real.txt".to_string()));
    assert!(!names.contains(&"dangling".to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn browse_relative_symlink_escaping_skipped() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(root.path().join("outside.txt"), "s").unwrap();
    // relative symlink that escapes the project via ".."
    std::os::unix::fs::symlink("../outside.txt", proj.join("escape_rel")).unwrap();
    std::fs::write(proj.join("keep.txt"), "k").unwrap();
    let state = state_dir(&proj);
    let (st, body) = parts(
        browse_files(State(state), auth(), Query(FileBrowseQuery { path: String::new() })).await,
    ).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let names: Vec<_> = json(&body)["entries"].as_array().unwrap().iter()
        .map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"keep.txt".to_string()));
    assert!(!names.contains(&"escape_rel".to_string()));
}

// ── reading a directory → read_to_string / read fails → Internal 500 ─

#[tokio::test]
async fn read_file_on_directory_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("adir")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file(State(state), auth(), Query(FileReadQuery { path: "adir".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn read_file_raw_on_directory_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("adir")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file_raw(State(state), auth(), Query(FileReadQuery { path: "adir".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

// ── rename: destination-parent escapes project → 400 ────────────────

#[tokio::test]
async fn rename_dest_parent_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    // an existing sibling dir OUTSIDE the project as the destination parent
    std::fs::create_dir(root.path().join("sibling")).unwrap();
    std::fs::write(proj.join("src.txt"), "x").unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        rename_entry(State(state), auth(),
            axum::Json(RenameRequest {
                from_path: "src.txt".into(),
                to_path: "../sibling/moved.txt".into(),
            })).await,
    ).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── rename: fs::rename itself fails (move dir into its own subpath) → 500 ─

#[tokio::test]
async fn rename_into_own_subpath_500() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("d")).unwrap();
    let state = state_dir(tmp.path());
    // dst parent "d" exists & is inside project, dst "d/sub" does not exist,
    // but renaming "d" into "d/sub" is EINVAL → Internal 500.
    let (st, _) = parts(
        rename_entry(State(state), auth(),
            axum::Json(RenameRequest { from_path: "d".into(), to_path: "d/sub".into() })).await,
    ).await;
    assert_eq!(st, ISE);
}

// ── upload: overwrite an already-existing file (target.canonicalize Ok) ─

async fn send_multipart(router: axum::Router, uri: &str, body: Vec<u8>) -> axum::http::StatusCode {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("POST").uri(uri)
        .header("content-type", "multipart/form-data; boundary=BOUND")
        .body(Body::from(body)).unwrap();
    router.oneshot(req).await.unwrap().status()
}

fn multipart_file(dir: Option<&str>, filename: &str, data: &str) -> Vec<u8> {
    let mut b = String::new();
    if let Some(d) = dir {
        b.push_str("--BOUND\r\nContent-Disposition: form-data; name=\"directory\"\r\n\r\n");
        b.push_str(d);
        b.push_str("\r\n");
    }
    b.push_str("--BOUND\r\n");
    b.push_str(&format!(
        "Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\r\n"
    ));
    b.push_str(data);
    b.push_str("\r\n--BOUND--\r\n");
    b.into_bytes()
}

#[tokio::test]
async fn upload_overwrite_existing_file_ok() {
    use crate::web::test_support::test_router;
    let tmp = tempfile::TempDir::new().unwrap();
    // Pre-existing file → target.canonicalize() succeeds (the Ok arm of or_else).
    std::fs::write(tmp.path().join("dup.txt"), "old").unwrap();
    let router = test_router(state_dir(tmp.path()));
    let st = send_multipart(router, "/api/file/upload", multipart_file(None, "dup.txt", "new")).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(std::fs::read_to_string(tmp.path().join("dup.txt")).unwrap(), "new");
}

#[tokio::test]
async fn upload_directory_dot_treated_as_root() {
    use crate::web::test_support::test_router;
    let tmp = tempfile::TempDir::new().unwrap();
    let router = test_router(state_dir(tmp.path()));
    // directory="." → the `upload_dir == "."` branch → written at root
    let st = send_multipart(router, "/api/file/upload", multipart_file(Some("."), "dot.txt", "x")).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(tmp.path().join("dot.txt").exists());
}
