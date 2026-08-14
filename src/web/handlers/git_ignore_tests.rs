//! `.gitignore` listing and appending.

use super::super::git_ext_handlers::git_ext_handlers_history_tests::{
    call, init_repo, state_for,
};
use axum::http::StatusCode;

#[tokio::test]
async fn gitignore_list_empty_when_absent() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "list"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["content"], "");
}

#[tokio::test]
async fn gitignore_default_action_is_list() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    // action "" → list branch.
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": ""})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["content"], "");
}

#[tokio::test]
async fn gitignore_add_creates_and_dedups() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);

    // Add to a non-existent .gitignore (creates it).
    let (s1, b1) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "add", "patterns": ["*.log", "target/"]})),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);
    let c1 = b1["content"].as_str().unwrap();
    assert!(c1.contains("*.log"));
    assert!(c1.contains("target/"));

    // Add again with a duplicate and a new one.
    let (s2, b2) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "add", "patterns": ["*.log", "dist/"]})),
    )
    .await;
    assert_eq!(s2, StatusCode::OK);
    let c2 = b2["content"].as_str().unwrap();
    assert_eq!(c2.matches("*.log").count(), 1);
    assert!(c2.contains("dist/"));

    // Now list should reflect the existing file (exists branch).
    let (s3, b3) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "list"})),
    )
    .await;
    assert_eq!(s3, StatusCode::OK);
    assert!(b3["content"].as_str().unwrap().contains("dist/"));
}

#[tokio::test]
async fn gitignore_add_appends_missing_trailing_newline() {
    let td = init_repo();
    let dir = td.path();
    // Pre-write a .gitignore WITHOUT trailing newline to hit that branch.
    std::fs::write(dir.join(".gitignore"), "foo").unwrap();
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "add", "patterns": ["bar"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let c = body["content"].as_str().unwrap();
    assert!(c.starts_with("foo\n"));
    assert!(c.contains("bar"));
}

#[tokio::test]
async fn gitignore_add_empty_patterns_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "add", "patterns": []})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn gitignore_unknown_action_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/gitignore",
        Some(serde_json::json!({"action": "delete"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
