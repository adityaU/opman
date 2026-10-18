use super::*;

// ── read_file / read_file_raw ──────────────────────────────────────

#[tokio::test]
async fn read_file_ok() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("main.rs"), "fn main() {}").expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, body) = parts(
        read_file(
            State(state),
            auth(),
            Query(FileReadQuery {
                path: "main.rs".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let v = json(&body);
    assert_eq!(v["content"], "fn main() {}");
    assert_eq!(v["language"], "rust");
}

#[tokio::test]
async fn read_file_missing_404() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file(
            State(state),
            auth(),
            Query(FileReadQuery {
                path: "ghost.rs".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn read_file_raw_ok_content_type() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("img.png"), [1u8, 2, 3]).expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let resp = read_file_raw(
        State(state),
        auth(),
        Query(FileReadQuery {
            path: "img.png".into(),
        }),
        axum::http::HeaderMap::new(),
    )
    .await
    .expect("test setup succeeds")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("test setup succeeds"),
        "image/png"
    );
}

#[tokio::test]
async fn read_file_raw_missing_404() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        read_file_raw(
            State(state),
            auth(),
            Query(FileReadQuery {
                path: "no.png".into(),
            }),
            axum::http::HeaderMap::new(),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

// ── write_file ─────────────────────────────────────────────────────

#[tokio::test]
async fn write_file_ok() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest {
                path: "new.txt".into(),
                content: "hello".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("new.txt")).expect("test setup succeeds"),
        "hello"
    );
}

#[tokio::test]
async fn write_file_missing_parent_404() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest {
                path: "no/such/dir/f.txt".into(),
                content: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn write_file_traversal_400() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    let state = state_dir(&proj);
    let (st, _) = parts(
        write_file(
            State(state),
            auth(),
            axum::Json(FileWriteRequest {
                path: "../evil.txt".into(),
                content: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

// ── create_file ────────────────────────────────────────────────────

#[tokio::test]
async fn create_file_ok_201() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest {
                path: "fresh.txt".into(),
                content: "c".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED);
    assert!(tmp.path().join("fresh.txt").exists());
}

#[tokio::test]
async fn create_file_already_exists_400() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::write(tmp.path().join("dup.txt"), "old").expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest {
                path: "dup.txt".into(),
                content: "new".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_file_missing_parent_404() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_file(
            State(state),
            auth(),
            axum::Json(FileCreateRequest {
                path: "ghostdir/f.txt".into(),
                content: "x".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::NOT_FOUND);
}

// ── create_dir ─────────────────────────────────────────────────────

#[tokio::test]
async fn create_dir_ok_nested_201() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_dir(
            State(state),
            auth(),
            axum::Json(DirCreateRequest {
                path: "a/b/c".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::CREATED);
    assert!(tmp.path().join("a/b/c").is_dir());
}

#[tokio::test]
async fn create_dir_already_exists_400() {
    let tmp = tempfile::TempDir::new().expect("test setup succeeds");
    std::fs::create_dir(tmp.path().join("exists")).expect("test setup succeeds");
    let state = state_dir(tmp.path());
    let (st, _) = parts(
        create_dir(
            State(state),
            auth(),
            axum::Json(DirCreateRequest {
                path: "exists".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_dir_traversal_400() {
    let root = tempfile::TempDir::new().expect("test setup succeeds");
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).expect("test setup succeeds");
    let state = state_dir(&proj);
    let (st, _) = parts(
        create_dir(
            State(state),
            auth(),
            axum::Json(DirCreateRequest {
                path: "../sneaky".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}
