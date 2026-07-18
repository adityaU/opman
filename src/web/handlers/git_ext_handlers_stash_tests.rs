//! Generated coverage tests for git_ext_handlers.rs (part 2):
//! git_pull, git_stash, git_gitignore, combined_output helper.
#![allow(clippy::disallowed_names)]

use super::*;
use super::git_ext_handlers_history_tests::{
    call, commit_all, init_repo, run_git, state_for, write_file,
};
use axum::http::StatusCode;
use std::path::Path;

// ── combined_output helper (direct) ──────────────────────────────────

#[test]
fn combined_output_stdout_only_and_with_stderr() {
    let base = run_git(Path::new("."), &["--version"]);
    let out = std::process::Output {
        status: base.status,
        stdout: b"  hello  ".to_vec(),
        stderr: Vec::new(),
    };
    assert_eq!(combined_output(&out), "hello");

    let base2 = run_git(Path::new("."), &["--version"]);
    let out2 = std::process::Output {
        status: base2.status,
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
    };
    assert_eq!(combined_output(&out2), "out\nerr");
}

// ── git_pull ─────────────────────────────────────────────────────────

#[tokio::test]
async fn pull_default_remote_no_origin_fails_gracefully() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    // remote empty → defaults to "origin"; no origin configured → success:false.
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/pull",
        Some(serde_json::json!({"remote": "", "branch": ""})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], false);
    assert!(!body["output"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn pull_named_remote_and_branch() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    // Non-empty (valid) remote + branch exercise the validate_git_ref branches.
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/pull",
        Some(serde_json::json!({"remote": "upstream", "branch": "main"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], false);
}

#[tokio::test]
async fn pull_invalid_remote_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/pull",
        Some(serde_json::json!({"remote": "-bad"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pull_invalid_branch_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "x\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/pull",
        Some(serde_json::json!({"remote": "origin", "branch": "a:b"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── git_stash ────────────────────────────────────────────────────────

#[tokio::test]
async fn stash_push_pop_list_drop_cycle() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");

    let state = state_for(dir);

    // push with message
    write_file(dir, "a.txt", "two\n");
    let (s_push, b_push) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "push", "message": "wip"})),
    )
    .await;
    assert_eq!(s_push, StatusCode::OK);
    assert_eq!(b_push["success"], true);

    // list
    let (s_list, b_list) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "list"})),
    )
    .await;
    assert_eq!(s_list, StatusCode::OK);
    assert_eq!(b_list["entries"].as_array().unwrap().len(), 1);
    assert_eq!(b_list["entries"][0]["reference"], "stash@{0}");

    // pop with explicit ref
    let (s_pop, b_pop) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "pop", "stash_ref": "stash@{0}"})),
    )
    .await;
    assert_eq!(s_pop, StatusCode::OK);
    assert_eq!(b_pop["success"], true);
}

#[tokio::test]
async fn stash_push_default_action_and_drop() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");
    let state = state_for(dir);

    // action "" → treated as push (no changes: still succeeds/no-op).
    write_file(dir, "a.txt", "two\n");
    let (s1, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": ""})),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    // drop without explicit ref (drops most recent).
    let (s2, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "drop"})),
    )
    .await;
    assert_eq!(s2, StatusCode::OK);
}

#[tokio::test]
async fn stash_drop_with_ref() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "one\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    write_file(dir, "a.txt", "two\n");
    run_git(dir, &["stash", "push", "-m", "x"]);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "drop", "stash_ref": "stash@{0}"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn stash_push_dash_message_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "push", "message": "-oops"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn stash_pop_dash_ref_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "pop", "stash_ref": "-x"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn stash_unknown_action_rejected() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, _) = call(
        &state,
        "POST",
        "/api/git/stash",
        Some(serde_json::json!({"action": "frobnicate"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── git_gitignore ────────────────────────────────────────────────────

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
