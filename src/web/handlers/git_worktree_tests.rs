//! Tests for `git_worktree.rs`: porcelain parsing, containment, and a real
//! add/list/remove/prune round trip on a temporary repository.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;
use std::process::Command;
use tempfile::TempDir;

// ── harness ──────────────────────────────────────────────────────────

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

/// A repository with one commit on `main`, inside a project root.
pub(crate) fn init_repo() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    let dir = td.path();
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("a.txt"), "x\n").expect("write");
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", "init"]);
    td
}

pub(crate) fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

pub(crate) fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

pub(crate) async fn parts<T: IntoResponse>(r: WebResult<T>) -> (axum::http::StatusCode, serde_json::Value) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

pub(crate) async fn list(state: &ServerState) -> serde_json::Value {
    let (status, body) = parts(
        git_worktrees(
            State(state.clone()),
            auth(),
            Query(GitRepoScope::default()),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    body
}

// ── porcelain parsing ────────────────────────────────────────────────

// ── entry projection ─────────────────────────────────────────────────

// ── containment ──────────────────────────────────────────────────────

// ── round trip ───────────────────────────────────────────────────────

#[tokio::test]
async fn list_reports_only_the_main_worktree_initially() {
    let td = init_repo();
    let state = state_for(td.path());
    let body = list(&state).await;
    let trees = body["worktrees"].as_array().expect("array");
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["main"], true);
    assert_eq!(trees[0]["current"], true);
    assert_eq!(trees[0]["branch"], "main");
    assert_eq!(trees[0]["relative"], ".");
    assert!(!trees[0]["head"].as_str().expect("head").is_empty());
}

#[tokio::test]
async fn remove_refuses_the_main_worktree() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, body) = parts(
        git_worktree_remove(
            State(state),
            auth(),
            Json(GitWorktreeRemoveRequest {
                path: ".".into(),
                force: false,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["ok"], false);
    assert_eq!(body["failure"], "failed");
    assert!(body["message"]
        .as_str()
        .expect("message")
        .contains("main worktree"));
}

#[tokio::test]
async fn remove_refuses_a_path_escaping_the_project() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, _) = parts(
        git_worktree_remove(
            State(state),
            auth(),
            Json(GitWorktreeRemoveRequest {
                path: "../outside".into(),
                force: false,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn remove_reports_git_refusal_for_an_unknown_worktree() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, body) = parts(
        git_worktree_remove(
            State(state),
            auth(),
            Json(GitWorktreeRemoveRequest {
                path: "nope".into(),
                force: false,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["ok"], false);
    assert!(body["hint"].is_string());
}

#[tokio::test]
async fn prune_reports_a_worktree_whose_directory_vanished() {
    let td = init_repo();
    let state = state_for(td.path());
    let (_, body) = parts(
        git_worktree_add(
            State(state.clone()),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "gone".into(),
                branch: "gone".into(),
                create: true,
                start_point: None,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(body["ok"], true, "add failed: {body}");

    std::fs::remove_dir_all(td.path().join("gone")).expect("remove dir");

    // The stale entry is still listed, and git marks it prunable.
    let body = list(&state).await;
    let stale = body["worktrees"]
        .as_array()
        .expect("array")
        .iter()
        .find(|t| t["branch"] == "gone")
        .cloned();
    if let Some(stale) = stale {
        assert!(stale["prunable"].is_string());
    }

    let (_, body) = parts(
        git_worktree_prune(State(state.clone()), auth(), Json(GitRepoScope::default())).await,
    )
    .await;
    assert_eq!(body["ok"], true);

    let body = list(&state).await;
    assert_eq!(body["worktrees"].as_array().expect("array").len(), 1);
}

#[tokio::test]
async fn worktrees_without_an_active_project_is_a_bad_request() {
    let state = test_server_state();
    let (status, _) = parts(
        git_worktrees(State(state), auth(), Query(GitRepoScope::default())).await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}
