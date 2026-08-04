//! Generated tests for the MCP editor tool handlers.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

/// A ServerState whose active project directory is `dir`.
fn state_with_dir(dir: PathBuf) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("p".to_string(), dir)]);
    s
}

// ── handle_editor_open ──────────────────────────────────────────────

#[tokio::test]
async fn open_missing_path_errors() {
    let s = test_server_state();
    let err = handle_editor_open(&s, &serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err.contains("Missing required 'path'"));
}

#[tokio::test]
async fn open_absolute_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hi").unwrap();
    let s = test_server_state();
    let msg = handle_editor_open(&s, &serde_json::json!({"path": f.to_string_lossy()}))
        .await
        .unwrap();
    assert!(msg.starts_with("Opened '"));
    assert!(!msg.contains("line"));
}

#[tokio::test]
async fn open_absolute_with_line() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hi").unwrap();
    let s = test_server_state();
    let msg = handle_editor_open(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "line": 12}),
    )
    .await
    .unwrap();
    assert!(msg.contains("at line 12"));
}

#[tokio::test]
async fn open_absolute_missing_file_errors() {
    let s = test_server_state();
    let err = handle_editor_open(&s, &serde_json::json!({"path": "/nope/xyz/absent.rs"}))
        .await
        .unwrap_err();
    assert!(err.contains("File not found"));
}

#[tokio::test]
async fn open_relative_resolves_against_working_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("rel.txt"), "x").unwrap();
    let s = state_with_dir(dir.path().to_path_buf());
    let msg = handle_editor_open(&s, &serde_json::json!({"path": "rel.txt"}))
        .await
        .unwrap();
    assert!(msg.contains("rel.txt"));
}

#[tokio::test]
async fn open_relative_without_working_dir_errors() {
    // No project -> get_working_dir() is None -> path used as-is (doesn't exist).
    let s = test_server_state();
    let err = handle_editor_open(&s, &serde_json::json!({"path": "still-relative.txt"}))
        .await
        .unwrap_err();
    assert!(err.contains("File not found"));
}

// ── handle_editor_read ──────────────────────────────────────────────

fn write_lines(dir: &std::path::Path, name: &str) -> PathBuf {
    let f = dir.join(name);
    std::fs::write(&f, "l1\nl2\nl3\nl4\nl5").unwrap();
    f
}

#[tokio::test]
async fn read_missing_path_errors() {
    let s = test_server_state();
    assert!(handle_editor_read(&s, &serde_json::json!({}))
        .await
        .is_err());
}

#[tokio::test]
async fn read_full_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    let out = handle_editor_read(&s, &serde_json::json!({"path": f.to_string_lossy()}))
        .await
        .unwrap();
    assert_eq!(out, "l1\nl2\nl3\nl4\nl5");
}

#[tokio::test]
async fn read_start_and_end_range() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    let out = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "start_line": 2, "end_line": 4}),
    )
    .await
    .unwrap();
    assert_eq!(out, "l2\nl3\nl4");
}

#[tokio::test]
async fn read_start_only() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    let out = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "start_line": 4}),
    )
    .await
    .unwrap();
    assert_eq!(out, "l4\nl5");
}

#[tokio::test]
async fn read_end_only() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    let out = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "end_line": 2}),
    )
    .await
    .unwrap();
    assert_eq!(out, "l1\nl2");
}

#[tokio::test]
async fn read_end_beyond_eof_clamps() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    let out = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "end_line": 999}),
    )
    .await
    .unwrap();
    assert_eq!(out, "l1\nl2\nl3\nl4\nl5");
}

#[tokio::test]
async fn read_start_past_eof_errors_both_variants() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_lines(dir.path(), "r.txt");
    let s = test_server_state();
    // start+end
    let e1 = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "start_line": 100, "end_line": 200}),
    )
    .await
    .unwrap_err();
    assert!(e1.contains("past end of file"));
    // start only
    let e2 = handle_editor_read(
        &s,
        &serde_json::json!({"path": f.to_string_lossy(), "start_line": 100}),
    )
    .await
    .unwrap_err();
    assert!(e2.contains("past end of file"));
}

#[tokio::test]
async fn read_nonexistent_file_errors() {
    let s = test_server_state();
    let err = handle_editor_read(&s, &serde_json::json!({"path": "/nope/absent.rs"}))
        .await
        .unwrap_err();
    assert!(err.contains("Failed to read"));
}

#[tokio::test]
async fn read_relative_with_working_dir() {
    let dir = tempfile::tempdir().unwrap();
    write_lines(dir.path(), "rel.txt");
    let s = state_with_dir(dir.path().to_path_buf());
    let out = handle_editor_read(
        &s,
        &serde_json::json!({"path": "rel.txt", "start_line": 1, "end_line": 1}),
    )
    .await
    .unwrap();
    assert_eq!(out, "l1");
}

// ── handle_editor_list ──────────────────────────────────────────────

#[tokio::test]
async fn list_no_working_dir_errors() {
    let s = test_server_state();
    let err = handle_editor_list(&s, &serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(err.contains("No active project directory"));
}

#[tokio::test]
async fn list_walks_tree_and_skips_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("top.rs"), "").unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src").join("lib.rs"), "").unwrap();
    // Ignored / hidden entries must be skipped.
    std::fs::create_dir(root.join("node_modules")).unwrap();
    std::fs::write(root.join("node_modules").join("x.js"), "").unwrap();
    std::fs::create_dir(root.join("target")).unwrap();
    std::fs::write(root.join(".hidden"), "").unwrap();

    let s = state_with_dir(root.to_path_buf());
    let out = handle_editor_list(&s, &serde_json::json!({}))
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let files: Vec<String> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect();
    assert!(files.iter().any(|f| f == "top.rs"));
    assert!(files.iter().any(|f| f == "src/"));
    assert!(files.iter().any(|f| f.ends_with("lib.rs")));
    assert!(!files.iter().any(|f| f.contains("node_modules")));
    assert!(!files.iter().any(|f| f.contains("target")));
    assert!(!files.iter().any(|f| f.contains(".hidden")));
    assert_eq!(v["count"], files.len() as u64);
}

#[tokio::test]
async fn list_depth_zero_does_not_descend() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join("sub")).unwrap();
    std::fs::write(root.join("sub").join("deep.rs"), "").unwrap();
    let s = state_with_dir(root.to_path_buf());
    let out = handle_editor_list(&s, &serde_json::json!({"depth": 0}))
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let files: Vec<String> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect();
    // The directory itself is listed, but its contents (depth 1) are not.
    assert!(files.iter().any(|f| f == "sub/"));
    assert!(!files.iter().any(|f| f.contains("deep.rs")));
}

#[tokio::test]
async fn list_nonexistent_subpath_errors() {
    let dir = tempfile::tempdir().unwrap();
    let s = state_with_dir(dir.path().to_path_buf());
    let err = handle_editor_list(&s, &serde_json::json!({"path": "does-not-exist"}))
        .await
        .unwrap_err();
    assert!(err.contains("Directory not found"));
}

#[tokio::test]
async fn list_subpath_pointing_at_file_yields_empty() {
    // base_dir.exists() is true but read_dir fails on a file -> empty listing.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x").unwrap();
    let s = state_with_dir(dir.path().to_path_buf());
    let out = handle_editor_list(&s, &serde_json::json!({"path": "f.txt"}))
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["count"], 0);
}
