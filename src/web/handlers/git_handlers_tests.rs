//! Generated coverage tests for git_handlers.rs.
//!
//! Strategy: build real temporary git repos with the `git` CLI (the handlers
//! themselves shell out to git, so it is guaranteed present), point a test
//! `ServerState` at the repo via `WebStateHandle::new_test_with_projects`, then
//! drive every endpoint through the production router.
#![allow(clippy::disallowed_names)]

use super::*;
use axum::http::StatusCode;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// ── test git repo helpers ────────────────────────────────────────────

fn run_git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("failed to spawn git")
}

/// Initialise an empty temp repo with a deterministic `main` branch and a local
/// identity so commits succeed regardless of the developer's global config.
fn init_repo() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    let dir = td.path();
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "commit.gpgsign", "false"]);
    td
}

fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write file");
}

fn commit_all(dir: &Path, msg: &str) {
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", msg]);
}

fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

async fn call(
    state: &ServerState,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let router = test_router(state.clone());
    let (status, bytes) = send_json(router, method, uri, body).await;
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

// ── git_status ───────────────────────────────────────────────────────

#[tokio::test]
async fn status_reports_staged_unstaged_untracked() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");

    // Modify tracked file (unstaged)
    write_file(dir, "a.txt", "changed\n");
    // Stage a new file (staged), then modify it again (also unstaged)
    write_file(dir, "b.txt", "b\n");
    run_git(dir, &["add", "b.txt"]);
    write_file(dir, "b.txt", "b2\n");
    // Untracked file
    write_file(dir, "c.txt", "c\n");

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/status?repo=.", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["branch"], "main");
    let staged: Vec<String> = body["staged"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect();
    let unstaged: Vec<String> = body["unstaged"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect();
    let untracked: Vec<String> = body["untracked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect();
    assert!(staged.contains(&"b.txt".to_string()));
    assert!(unstaged.contains(&"a.txt".to_string()));
    assert!(unstaged.contains(&"b.txt".to_string()));
    assert!(untracked.contains(&"c.txt".to_string()));
}

#[tokio::test]
async fn status_with_repo_scope_subdir() {
    // base is a plain dir containing a nested git repo `sub`.
    let base = TempDir::new().unwrap();
    let sub = base.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    run_git(&sub, &["init", "-q"]);
    run_git(&sub, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(&sub, &["config", "user.name", "T"]);
    run_git(&sub, &["config", "user.email", "t@e.com"]);
    write_file(&sub, "f.txt", "x\n");
    commit_all(&sub, "init");

    let state = state_for(base.path());
    let (status, body) = call(&state, "GET", "/api/git/status?repo=sub", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["branch"], "main");
}

#[tokio::test]
async fn status_no_active_project_is_bad_request() {
    // Default test state has no projects → resolve_project_dir errors.
    let state = test_server_state();
    let (status, _) = call(&state, "GET", "/api/git/status", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn status_repo_not_a_git_repo_is_bad_request() {
    let td = init_repo();
    std::fs::create_dir(td.path().join("plain")).unwrap();
    let state = state_for(td.path());
    let (status, _) = call(&state, "GET", "/api/git/status?repo=plain", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn status_repo_missing_is_not_found() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, _) = call(&state, "GET", "/api/git/status?repo=nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn status_repo_traversal_is_bad_request() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, _) = call(&state, "GET", "/api/git/status?repo=../..", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── git_diff ─────────────────────────────────────────────────────────

#[tokio::test]
async fn diff_unstaged_shows_changes() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");
    write_file(dir, "a.txt", "two\n");

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/diff", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["diff"].as_str().unwrap().contains("a.txt"));
}

#[tokio::test]
async fn diff_staged_and_file_filter() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");
    write_file(dir, "a.txt", "two\n");
    run_git(dir, &["add", "a.txt"]);

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/diff?staged=true", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["diff"].as_str().unwrap().contains("two"));

    let (status2, body2) =
        call(&state, "GET", "/api/git/diff?staged=true&file=a.txt", None).await;
    assert_eq!(status2, StatusCode::OK);
    assert!(body2["diff"].as_str().unwrap().contains("a.txt"));
}

// ── git_log ──────────────────────────────────────────────────────────

#[tokio::test]
async fn log_returns_commits_and_respects_limit() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "first");
    write_file(dir, "a.txt", "2\n");
    commit_all(dir, "second");

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/log", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["commits"].as_array().unwrap().len(), 2);

    let (status2, body2) = call(&state, "GET", "/api/git/log?limit=1", None).await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(body2["commits"].as_array().unwrap().len(), 1);
    assert_eq!(body2["commits"][0]["message"], "second");
}

// ── git_stage ────────────────────────────────────────────────────────

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

// ── git_unstage ──────────────────────────────────────────────────────

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

// ── git_commit ───────────────────────────────────────────────────────

#[tokio::test]
async fn commit_empty_message_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "   "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn commit_dash_message_rejected() {
    let td = init_repo();
    let dir = td.path();
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "-oops"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn commit_success_returns_hash() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    run_git(dir, &["add", "-A"]);
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "my commit"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["hash"].as_str().unwrap().is_empty());
    assert_eq!(body["message"], "my commit");
}

#[tokio::test]
async fn commit_nothing_staged_fails() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    // Clean tree now.
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "nothing"})),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// ── git_discard ──────────────────────────────────────────────────────

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
