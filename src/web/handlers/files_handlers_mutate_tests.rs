//! Generated coverage tests for `files_handlers.rs` — delete/rename/upload/search handlers.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::{send_json, test_router, test_server_state};
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

// ── delete_file ────────────────────────────────────────────────────

#[tokio::test]
async fn delete_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("gone.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_file(
            State(state),
            auth(),
            axum::Json(FileDeleteRequest {
                path: "gone.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(!tmp.path().join("gone.txt").exists());
}

#[tokio::test]
async fn delete_file_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_file(
            State(state),
            auth(),
            axum::Json(FileDeleteRequest {
                path: "ghost".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_file_on_dir_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("adir")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_file(
            State(state),
            auth(),
            axum::Json(FileDeleteRequest {
                path: "adir".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_file_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let outside = root.path().join("out.txt");
    std::fs::write(&outside, "x").unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        delete_file(
            State(state),
            auth(),
            axum::Json(FileDeleteRequest {
                path: "../out.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── delete_dir ─────────────────────────────────────────────────────

#[tokio::test]
async fn delete_dir_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("d/inner")).unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_dir(
            State(state),
            auth(),
            axum::Json(DirDeleteRequest { path: "d".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(!tmp.path().join("d").exists());
}

#[tokio::test]
async fn delete_dir_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_dir(
            State(state),
            auth(),
            axum::Json(DirDeleteRequest {
                path: "ghostdir".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_dir_root_forbidden_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_dir(
            State(state),
            auth(),
            axum::Json(DirDeleteRequest { path: ".".into() }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_dir_on_file_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("afile.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        delete_dir(
            State(state),
            auth(),
            axum::Json(DirDeleteRequest {
                path: "afile.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_dir_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::create_dir(root.path().join("outdir")).unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        delete_dir(
            State(state),
            auth(),
            axum::Json(DirDeleteRequest {
                path: "../outdir".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── rename_entry ───────────────────────────────────────────────────

#[tokio::test]
async fn rename_empty_paths_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: String::new(),
                to_path: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_same_path_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "a".into(),
                to_path: "a".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
}

#[tokio::test]
async fn rename_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("old.txt"), "data").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "old.txt".into(),
                to_path: "new.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(tmp.path().join("new.txt").exists());
    assert!(!tmp.path().join("old.txt").exists());
}

#[tokio::test]
async fn rename_source_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "ghost.txt".into(),
                to_path: "new.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rename_dest_exists_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("src.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("dst.txt"), "b").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "src.txt".into(),
                to_path: "dst.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_project_root_forbidden_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: ".".into(),
                to_path: "renamed".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_source_traversal_400() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(root.path().join("outside.txt"), "x").unwrap();
    let state = state_dir(&proj);
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "../outside.txt".into(),
                to_path: "here.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_dest_parent_missing_404() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("s.txt"), "x").unwrap();
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "s.txt".into(),
                to_path: "nodir/d.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

// ── search_files (async handler) ───────────────────────────────────

#[tokio::test]
async fn search_files_empty_query_returns_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let resp = search_files(
        State(state),
        auth(),
        Query(FileSearchQuery {
            q: "   ".into(),
            limit: 10,
        }),
    )
    .await
    .unwrap()
    .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_files_with_matches() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("findme.txt"), "").unwrap();
    let state = state_dir(tmp.path());
    let resp = search_files(
        State(state),
        auth(),
        Query(FileSearchQuery {
            q: "findme".into(),
            limit: 100,
        }),
    )
    .await
    .unwrap()
    .into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["query"], "findme");
    assert!(v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "findme.txt"));
}

// ── upload_files (via router / multipart) ──────────────────────────

async fn send_multipart(
    router: axum::Router,
    uri: &str,
    body: Vec<u8>,
) -> (axum::http::StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "multipart/form-data; boundary=BOUND")
        .body(Body::from(body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn multipart_file(dir: Option<&str>, field: &str, filename: Option<&str>, data: &str) -> Vec<u8> {
    let mut b = String::new();
    if let Some(d) = dir {
        b.push_str("--BOUND\r\n");
        b.push_str("Content-Disposition: form-data; name=\"directory\"\r\n\r\n");
        b.push_str(d);
        b.push_str("\r\n");
    }
    b.push_str("--BOUND\r\n");
    match filename {
        Some(fname) => b.push_str(&format!(
            "Content-Disposition: form-data; name=\"{field}\"; filename=\"{fname}\"\r\n\r\n"
        )),
        None => b.push_str(&format!(
            "Content-Disposition: form-data; name=\"{field}\"\r\n\r\n"
        )),
    }
    b.push_str(data);
    b.push_str("\r\n--BOUND--\r\n");
    b.into_bytes()
}

#[tokio::test]
async fn upload_single_file_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let router = test_router(state);
    let body = multipart_file(None, "file", Some("up.txt"), "content-here");
    let (st, resp_body) = send_multipart(router, "/api/file/upload", body).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
    assert_eq!(v["files"][0], "up.txt");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("up.txt")).unwrap(),
        "content-here"
    );
}

#[tokio::test]
async fn upload_with_directory() {
    let tmp = tempfile::TempDir::new().unwrap();
    // The handler canonicalizes the target's parent before creating it, so the
    // sub-directory must already exist for the nested-upload success path.
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    let state = state_dir(tmp.path());
    let router = test_router(state);
    let body = multipart_file(Some("sub"), "file", Some("d.txt"), "x");
    let (st, _) = send_multipart(router, "/api/file/upload", body).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(tmp.path().join("sub/d.txt").exists());
}

#[tokio::test]
async fn upload_field_without_filename_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let router = test_router(state);
    // a non-"directory" field with no filename → rejected
    let body = multipart_file(None, "file", None, "x");
    let (st, _) = send_multipart(router, "/api/file/upload", body).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_only_directory_no_files_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let router = test_router(state);
    // only a directory field, no file fields → "No files in upload"
    let mut b = String::new();
    b.push_str("--BOUND\r\n");
    b.push_str("Content-Disposition: form-data; name=\"directory\"\r\n\r\n");
    b.push_str("sub");
    b.push_str("\r\n--BOUND--\r\n");
    let (st, _) = send_multipart(router, "/api/file/upload", b.into_bytes()).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_unsafe_filename_400() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let router = test_router(state);
    // filename "." → sanitizes to "." which starts with '.' → Invalid filename
    let body = multipart_file(None, "file", Some("."), "x");
    let (st, _) = send_multipart(router, "/api/file/upload", body).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_no_project_400() {
    let state = test_server_state();
    let router = test_router(state);
    let body = multipart_file(None, "file", Some("up.txt"), "x");
    let (st, _) = send_multipart(router, "/api/file/upload", body).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// A basic router smoke test through send_json for the file browse endpoint
// (exercises routing + query extraction).
#[tokio::test]
async fn browse_via_router_no_project() {
    let state = test_server_state();
    let router = test_router(state);
    let (st, _) = send_json(router, "GET", "/api/files?path=", None).await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
