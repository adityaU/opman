//! Generated coverage tests for git_ext_handlers.rs (part 1):
//! validators, git_show, git_branches, git_checkout, git_range_diff.
#![allow(clippy::disallowed_names)]

use super::*;
use axum::http::StatusCode;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// ── shared helpers ───────────────────────────────────────────────────

pub(crate) fn run_git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("failed to spawn git")
}

pub(crate) fn init_repo() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    let dir = td.path();
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "commit.gpgsign", "false"]);
    td
}

pub(crate) fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write file");
}

pub(crate) fn commit_all(dir: &Path, msg: &str) {
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", msg]);
}

pub(crate) fn head_hash(dir: &Path) -> String {
    let out = run_git(dir, &["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub(crate) fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

pub(crate) async fn call(
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

// ── validators (direct) ──────────────────────────────────────────────

#[test]
fn validate_git_hash_variants() {
    assert!(validate_git_hash("abc123def").is_ok());
    assert!(validate_git_hash("").is_err());
    assert!(validate_git_hash("xyz!!").is_err());
    assert!(validate_git_hash("123 456").is_err());
}

#[test]
fn validate_git_ref_variants() {
    assert!(validate_git_ref("main").is_ok());
    assert!(validate_git_ref("feature/foo").is_ok());
    assert!(validate_git_ref("").is_err());
    assert!(validate_git_ref("-x").is_err());
    assert!(validate_git_ref("a..b").is_err());
    assert!(validate_git_ref("a~1").is_err());
    assert!(validate_git_ref("a^").is_err());
    assert!(validate_git_ref("a:b").is_err());
}

#[test]
fn validate_git_filename_variants() {
    assert!(validate_git_filename("file.txt").is_ok());
    assert!(validate_git_filename("").is_err());
    assert!(validate_git_filename("-rf").is_err());
}

// ── git_show ─────────────────────────────────────────────────────────

#[tokio::test]
async fn show_valid_commit() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "hello\n");
    commit_all(dir, "add a");
    let hash = head_hash(dir);

    let state = state_for(dir);
    let (status, body) =
        call(&state, "GET", &format!("/api/git/show?hash={hash}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "add a");
    assert!(body["diff"].as_str().unwrap().contains("a.txt"));
    let files = body["files"].as_array().unwrap();
    assert!(files.iter().any(|f| f["path"] == "a.txt"));
}

#[tokio::test]
async fn show_invalid_hash_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(&state, "GET", "/api/git/show?hash=zzz!!", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn show_unknown_hash_fails() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    // Valid hex but not an existing object.
    let (status, _) = call(
        &state,
        "GET",
        "/api/git/show?hash=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── git_branches ─────────────────────────────────────────────────────

#[tokio::test]
async fn branches_local_and_current() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    run_git(dir, &["branch", "feature"]);

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/branches", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["current"], "main");
    let local: Vec<String> = body["local"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    assert!(local.contains(&"main".to_string()));
    assert!(local.contains(&"feature".to_string()));
}

#[tokio::test]
async fn branches_with_remote() {
    // Origin repo with a commit.
    let origin = init_repo();
    write_file(origin.path(), "a.txt", "x\n");
    commit_all(origin.path(), "init");

    // Clone-like: second repo that fetches from origin.
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let origin_url = origin.path().to_string_lossy().to_string();
    run_git(dir, &["remote", "add", "origin", &origin_url]);
    run_git(dir, &["fetch", "-q", "origin"]);
    run_git(dir, &["remote", "set-head", "origin", "-a"]);

    let state = state_for(dir);
    let (status, body) = call(&state, "GET", "/api/git/branches", None).await;
    assert_eq!(status, StatusCode::OK);
    let remote: Vec<String> = body["remote"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    // origin/main should be present; origin/HEAD filtered out.
    assert!(remote.iter().any(|r| r.contains("origin/main")));
    assert!(!remote.iter().any(|r| r.contains("HEAD")));
}

// ── git_checkout ─────────────────────────────────────────────────────

#[tokio::test]
async fn checkout_existing_branch_succeeds() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    run_git(dir, &["branch", "feature"]);
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/checkout",
        Some(serde_json::json!({"branch": "feature"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["branch"], "feature");
}

#[tokio::test]
async fn checkout_invalid_ref_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/checkout",
        Some(serde_json::json!({"branch": "a:b"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn checkout_nonexistent_branch_reports_failure() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/checkout",
        Some(serde_json::json!({"branch": "ghostbranch"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], false);
    assert!(body["message"].as_str().unwrap().len() > 0);
}

// ── git_range_diff ───────────────────────────────────────────────────

#[tokio::test]
async fn range_diff_between_base_and_head() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "base\n");
    commit_all(dir, "base commit");
    // Create a feature branch with an extra commit.
    run_git(dir, &["checkout", "-q", "-b", "feature"]);
    write_file(dir, "a.txt", "feature change\n");
    commit_all(dir, "feature commit");

    let state = state_for(dir);
    let (status, body) =
        call(&state, "GET", "/api/git/range-diff?base=main", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["branch"], "feature");
    assert_eq!(body["base"], "main");
    assert_eq!(body["commits"].as_array().unwrap().len(), 1);
    assert!(body["diff"].as_str().unwrap().contains("feature change"));
    assert!(body["files_changed"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn range_diff_default_base_empty_range() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    // Default base = "main"; HEAD is main → empty range but still 200.
    let (status, body) = call(&state, "GET", "/api/git/range-diff", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["base"], "main");
    assert_eq!(body["commits"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn range_diff_invalid_base_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) =
        call(&state, "GET", "/api/git/range-diff?base=a..b", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
