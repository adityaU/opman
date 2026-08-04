//! Generated tests for `doc_handlers.rs` (doc-read / doc-write endpoints).
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::web_state::WebStateHandle;
use axum::http::StatusCode;
use serde_json::json;

/// Build a `ServerState` whose active project working dir is `dir`.
fn state_with_dir(dir: &std::path::Path) -> crate::web::types::ServerState {
    let mut st = test_server_state();
    st.web_state =
        WebStateHandle::new_test_with_projects(vec![("proj".to_string(), dir.to_path_buf())]);
    st
}

fn build_xlsx(path: &std::path::Path) {
    use rust_xlsxwriter::Workbook;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("S1").unwrap();
    ws.write_string(0, 0, "Name").unwrap();
    ws.write_number(0, 1, 5.0).unwrap();
    wb.save(path).unwrap();
}

// ── doc_read ─────────────────────────────────────────────────────────

#[tokio::test]
async fn doc_read_no_project_bad_request() {
    let st = test_server_state();
    let router = test_router(st);
    let (status, _) = send_json(router, "GET", "/api/file/doc-read?path=book.xlsx", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn doc_read_xlsx_success() {
    let dir = tempfile::TempDir::new().unwrap();
    build_xlsx(&dir.path().join("book.xlsx"));
    let router = test_router(state_with_dir(dir.path()));
    let (status, body) = send_json(router, "GET", "/api/file/doc-read?path=book.xlsx", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["path"], "book.xlsx");
    assert_eq!(v["data"]["type"], "spreadsheet");
    assert_eq!(v["data"]["sheets"][0]["name"], "S1");
}

#[tokio::test]
async fn doc_read_tsv_success() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("d.tsv"), "a\tb\n1\t2\n").unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let (status, body) = send_json(router, "GET", "/api/file/doc-read?path=d.tsv", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["data"]["type"], "spreadsheet");
}

#[tokio::test]
async fn doc_read_docx_success() {
    use std::io::Write;
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("d.docx");
    let doc = "<?xml version=\"1.0\"?><w:document \
        xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
        <w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>";
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(doc.as_bytes()).unwrap();
    zip.finish().unwrap();

    let router = test_router(state_with_dir(dir.path()));
    let (status, body) = send_json(router, "GET", "/api/file/doc-read?path=d.docx", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["data"]["type"], "document");
    assert!(v["data"]["html"].as_str().unwrap().contains("Hi"));
}

#[tokio::test]
async fn doc_read_unsupported_extension() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("note.txt"), "hi").unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let (status, _) = send_json(router, "GET", "/api/file/doc-read?path=note.txt", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn doc_read_missing_file_not_found() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let (status, _) = send_json(router, "GET", "/api/file/doc-read?path=missing.xlsx", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn doc_read_path_traversal_rejected() {
    let dir = tempfile::TempDir::new().unwrap();
    // A real file outside the project base, reachable via "..".
    let base = dir.path().join("base");
    std::fs::create_dir(&base).unwrap();
    std::fs::write(dir.path().join("secret.xlsx"), "x").unwrap();
    let router = test_router(state_with_dir(&base));
    let (status, _) = send_json(
        router,
        "GET",
        "/api/file/doc-read?path=../secret.xlsx",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── doc_write ────────────────────────────────────────────────────────

#[tokio::test]
async fn doc_write_no_project_bad_request() {
    let st = test_server_state();
    let router = test_router(st);
    let body = json!({"path": "out.xlsx", "data": {"type": "spreadsheet", "sheets": []}});
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn doc_write_xlsx_success() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({
        "path": "out.xlsx",
        "data": {"type": "spreadsheet", "sheets": [{"name": "S", "rows": [["a", "b"]]}]}
    });
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(dir.path().join("out.xlsx").exists());
}

#[tokio::test]
async fn doc_write_tsv_success() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({
        "path": "out.tsv",
        "data": {"type": "spreadsheet", "sheets": [{"name": "S", "rows": [["x", "y"]]}]}
    });
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    let content = std::fs::read_to_string(dir.path().join("out.tsv")).unwrap();
    assert_eq!(content, "x\ty\n");
}

#[tokio::test]
async fn doc_write_docx_success() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({
        "path": "out.docx",
        "data": {"type": "document", "html": "<h1>Title</h1><p>hi</p>"}
    });
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(dir.path().join("out.docx").exists());
}

#[tokio::test]
async fn doc_write_unsupported_extension() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({"path": "out.bin", "data": {"type": "spreadsheet", "sheets": []}});
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn doc_write_parent_not_found() {
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({
        "path": "nope/out.xlsx",
        "data": {"type": "spreadsheet", "sheets": []}
    });
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn doc_write_path_traversal_rejected() {
    let dir = tempfile::TempDir::new().unwrap();
    let base = dir.path().join("base");
    std::fs::create_dir(&base).unwrap();
    let router = test_router(state_with_dir(&base));
    let body = json!({
        "path": "../evil.xlsx",
        "data": {"type": "spreadsheet", "sheets": [{"name": "S", "rows": [["a"]]}]}
    });
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!dir.path().join("evil.xlsx").exists());
}

#[tokio::test]
async fn doc_write_invalid_root_path() {
    // path "/" -> target has no parent -> "Invalid file path" BadRequest.
    let dir = tempfile::TempDir::new().unwrap();
    let router = test_router(state_with_dir(dir.path()));
    let body = json!({"path": "/", "data": {"type": "spreadsheet", "sheets": []}});
    let (status, _) = send_json(router, "POST", "/api/file/doc-write", Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
