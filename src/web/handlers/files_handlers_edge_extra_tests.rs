use super::*;

// ── reading a directory → read_to_string / read fails → Internal 500 ─

#[tokio::test]
async fn read_file_on_directory_500() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("adir")).expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file(
            State(state),
            auth(),
            Query(FileReadQuery {
                path: "adir".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

#[tokio::test]
async fn read_file_raw_on_directory_500() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("adir")).expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file_raw(
            State(state),
            auth(),
            Query(FileReadQuery {
                path: "adir".into(),
            }),
            axum::http::HeaderMap::new(),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

// ── rename: destination-parent escapes project → 400 ────────────────

#[tokio::test]
async fn rename_dest_parent_traversal_400() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    // an existing sibling dir OUTSIDE the project as the destination parent
    std::fs::create_dir(root.path().join("sibling")).expect("test setup succeeds");
    std::fs::write(proj.join("src.txt"), "x").expect("test setup succeeds");
    let state = state_dir(&proj);
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "src.txt".into(),
                to_path: "../sibling/moved.txt".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── rename: fs::rename itself fails (move dir into its own subpath) → 500 ─

#[tokio::test]
async fn rename_into_own_subpath_500() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("d")).expect("test setup succeeds");
    let state = state_dir(tmp.path());
    // dst parent "d" exists & is inside project, dst "d/sub" does not exist,
    // but renaming "d" into "d/sub" is EINVAL → Internal 500.
    let (st, _) = parts(
        rename_entry(
            State(state),
            auth(),
            axum::Json(RenameRequest {
                from_path: "d".into(),
                to_path: "d/sub".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, ISE);
}

// ── upload: overwrite an already-existing file (target.canonicalize Ok) ─

async fn send_multipart(router: axum::Router, uri: &str, body: Vec<u8>) -> axum::http::StatusCode {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "multipart/form-data; boundary=BOUND")
        .body(Body::from(body))
        .expect("test setup succeeds");
    router
        .oneshot(req)
        .await
        .expect("test setup succeeds")
        .status()
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
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    // Pre-existing file → target.canonicalize() succeeds (the Ok arm of or_else).
    std::fs::write(tmp.path().join("dup.txt"), "old").expect("test setup succeeds");
    let router = test_router(state_dir(tmp.path()));
    let st = send_multipart(
        router,
        "/api/file/upload",
        multipart_file(None, "dup.txt", "new"),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("dup.txt")).expect("test setup succeeds"),
        "new"
    );
}

#[tokio::test]
async fn upload_directory_dot_treated_as_root() {
    use crate::web::test_support::test_router;
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let router = test_router(state_dir(tmp.path()));
    // directory="." → the `upload_dir == "."` branch → written at root
    let st = send_multipart(
        router,
        "/api/file/upload",
        multipart_file(Some("."), "dot.txt", "x"),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert!(tmp.path().join("dot.txt").exists());
}
