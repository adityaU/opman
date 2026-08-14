//! Stage, unstage, commit and discard against real repositories.

use super::super::git_handlers::git_handlers_tests::{
    call, commit_all, init_repo, run_git, state_for, write_file,
};
use axum::http::StatusCode;

#[tokio::test]
async fn discard_empty_files_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/discard",
        Some(serde_json::json!({"files": []})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn discard_nonexistent_fails() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/discard",
        Some(serde_json::json!({"files": ["ghost.txt"]})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn discard_invalid_filename_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/discard",
        Some(serde_json::json!({"files": ["-x"]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unstage_nonexistent_fails() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/unstage",
        Some(serde_json::json!({"files": ["ghost.txt"]})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn discard_restores_modified_file() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "original\n");
    commit_all(dir, "init");
    write_file(dir, "a.txt", "modified\n");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/discard",
        Some(serde_json::json!({"files": ["a.txt"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let restored = std::fs::read_to_string(dir.join("a.txt")).unwrap();
    assert_eq!(restored, "original\n");
}

#[tokio::test]
async fn stage_nonexistent_file_fails() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stage",
        Some(serde_json::json!({"files": ["ghost.txt"]})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn stage_all_files() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    write_file(dir, "a.txt", "2\n");

    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stage",
        Some(serde_json::json!({"files": []})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unstage_specific_and_all() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    write_file(dir, "b.txt", "b\n");
    run_git(dir, &["add", "b.txt"]);
    let state = state_for(dir);

    let (s1, _) = call(
        &state,
        "POST",
        "/api/git/unstage",
        Some(serde_json::json!({"files": ["b.txt"]})),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    run_git(dir, &["add", "b.txt"]);
    let (s2, _) = call(
        &state,
        "POST",
        "/api/git/unstage",
        Some(serde_json::json!({"files": []})),
    )
    .await;
    assert_eq!(s2, StatusCode::OK);
}

#[tokio::test]
async fn stage_specific_file() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    write_file(dir, "b.txt", "b\n");

    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stage",
        Some(serde_json::json!({"files": ["b.txt"], "repo": "."})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unstage_invalid_filename() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/unstage",
        Some(serde_json::json!({"files": ["-bad"]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn stage_invalid_and_empty_filenames() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);

    let (s1, _) = call(
        &state,
        "POST",
        "/api/git/stage",
        Some(serde_json::json!({"files": ["-x"]})),
    )
    .await;
    assert_eq!(s1, StatusCode::BAD_REQUEST);

    let (s2, _) = call(
        &state,
        "POST",
        "/api/git/stage",
        Some(serde_json::json!({"files": [""]})),
    )
    .await;
    assert_eq!(s2, StatusCode::BAD_REQUEST);
}
