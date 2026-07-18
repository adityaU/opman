//! Generated coverage tests for git_context_handlers.rs:
//! git_context_summary + git_repos (discover_repos / quick_repo_info).
#![allow(clippy::disallowed_names)]

use super::*;
use axum::http::StatusCode;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

fn init_repo_at(dir: &Path) {
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "commit.gpgsign", "false"]);
}

fn init_repo() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    init_repo_at(td.path());
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

async fn call(state: &ServerState, uri: &str) -> (StatusCode, serde_json::Value) {
    let router = test_router(state.clone());
    let (status, bytes) = send_json(router, "GET", uri, None).await;
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

// ── git_context_summary ──────────────────────────────────────────────

#[tokio::test]
async fn context_summary_clean_tree() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, body) = call(&state, "/api/git/context-summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["branch"], "main");
    assert_eq!(body["recent_commits"].as_array().unwrap().len(), 1);
    assert_eq!(body["staged_count"], 0);
    assert_eq!(body["unstaged_count"], 0);
    assert_eq!(body["untracked_count"], 0);
    let summary = body["summary"].as_str().unwrap();
    assert!(summary.contains("Working tree clean"));
    assert!(summary.contains("Last commit: init"));
}

#[tokio::test]
async fn context_summary_with_changes() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");
    // staged change
    write_file(dir, "b.txt", "b\n");
    run_git(dir, &["add", "b.txt"]);
    // unstaged change to tracked file
    write_file(dir, "a.txt", "two\n");
    // untracked file
    write_file(dir, "c.txt", "c\n");

    let state = state_for(dir);
    let (status, body) = call(&state, "/api/git/context-summary").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["staged_count"].as_u64().unwrap() >= 1);
    assert!(body["unstaged_count"].as_u64().unwrap() >= 1);
    assert!(body["untracked_count"].as_u64().unwrap() >= 1);
    let summary = body["summary"].as_str().unwrap();
    assert!(summary.contains("staged"));
    assert!(summary.contains("unstaged"));
    assert!(summary.contains("untracked"));
    assert!(!summary.contains("Working tree clean"));
}

#[tokio::test]
async fn context_summary_empty_repo_no_commits() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, body) = call(&state, "/api/git/context-summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["recent_commits"].as_array().unwrap().len(), 0);
    // No commits and no files → clean.
    assert!(body["summary"].as_str().unwrap().contains("Working tree clean"));
}

#[tokio::test]
async fn context_summary_no_active_project() {
    let state = test_server_state();
    let (status, _) = call(&state, "/api/git/context-summary").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── git_repos ────────────────────────────────────────────────────────

#[tokio::test]
async fn repos_discovers_root_and_nested() {
    let base = init_repo();
    let dir = base.path();
    write_file(dir, "root.txt", "r\n");
    commit_all(dir, "root init");
    // Leave a change in the root so quick_repo_info counts are exercised.
    write_file(dir, "root.txt", "changed\n");
    write_file(dir, "untracked.txt", "u\n");

    // Nested repo under sub/
    let sub = dir.join("sub");
    std::fs::create_dir(&sub).unwrap();
    init_repo_at(&sub);
    write_file(&sub, "s.txt", "s\n");
    commit_all(&sub, "sub init");

    // A skipped directory containing a repo (must NOT be discovered).
    let nm = dir.join("node_modules").join("pkg");
    std::fs::create_dir_all(&nm).unwrap();
    init_repo_at(&nm);

    // A plain file at the top level (exercises the non-dir continue).
    write_file(dir, "plain.md", "hi\n");

    let state = state_for(dir);
    let (status, body) = call(&state, "/api/git/repos").await;
    assert_eq!(status, StatusCode::OK);
    let repos = body["repos"].as_array().unwrap();
    let paths: Vec<String> = repos
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_string())
        .collect();
    assert!(paths.contains(&".".to_string()));
    assert!(paths.contains(&"sub".to_string()));
    assert!(!paths.iter().any(|p| p.contains("node_modules")));

    // Root entry first (sorted), named after the dir.
    assert_eq!(repos[0]["path"], ".");
    // Root has an unstaged change + an untracked file.
    let root = &repos[0];
    assert!(root["unstaged_count"].as_u64().unwrap() >= 1);
    assert!(root["untracked_count"].as_u64().unwrap() >= 1);
    // Nested repo name is derived from its rel path.
    let sub_entry = repos.iter().find(|r| r["path"] == "sub").unwrap();
    assert_eq!(sub_entry["name"], "sub");
}

#[tokio::test]
async fn repos_respects_max_depth() {
    // base is NOT a git repo; a repo is buried deeper than max_depth (4).
    let base = TempDir::new().unwrap();
    let deep = base
        .path()
        .join("l1")
        .join("l2")
        .join("l3")
        .join("l4")
        .join("deep");
    std::fs::create_dir_all(&deep).unwrap();
    init_repo_at(&deep);

    let state = state_for(base.path());
    let (status, body) = call(&state, "/api/git/repos").await;
    assert_eq!(status, StatusCode::OK);
    let repos = body["repos"].as_array().unwrap();
    // The deeply-buried repo is beyond the depth limit → not discovered.
    assert!(!repos.iter().any(|r| r["path"].as_str().unwrap().contains("deep")));
}

#[tokio::test]
async fn repos_shallow_nested_only_root_absent_when_base_not_repo() {
    // base not a repo, one nested repo at depth 1 → found; root "." absent.
    let base = TempDir::new().unwrap();
    let sub = base.path().join("proj");
    std::fs::create_dir(&sub).unwrap();
    init_repo_at(&sub);
    write_file(&sub, "a.txt", "x\n");
    commit_all(&sub, "init");

    let state = state_for(base.path());
    let (status, body) = call(&state, "/api/git/repos").await;
    assert_eq!(status, StatusCode::OK);
    let repos = body["repos"].as_array().unwrap();
    let paths: Vec<String> = repos
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_string())
        .collect();
    assert!(paths.contains(&"proj".to_string()));
    assert!(!paths.contains(&".".to_string()));
}

#[tokio::test]
async fn repos_no_active_project() {
    let state = test_server_state();
    let (status, _) = call(&state, "/api/git/repos").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
