//! Tests for the sync handlers, driven against real temporary repositories.
//!
//! "Remotes" are always local clones, so nothing here touches the network.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

fn init_repo_at(dir: &Path) {
    run_git(dir, &["init", "-q"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "commit.gpgsign", "false"]);
}

pub(crate) fn init_repo() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    init_repo_at(td.path());
    td
}

pub(crate) fn commit(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write file");
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", name]);
}

pub(crate) fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

pub(crate) fn auth() -> AuthUser {
    AuthUser {
        subject: "test".to_string(),
    }
}

pub(crate) async fn body_of(response: impl IntoResponse) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_response().into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json body")
}

pub(crate) async fn status_of(dir: &Path) -> serde_json::Value {
    let state = state_for(dir);
    let response = git_sync_status(
        State(state),
        auth(),
        Query(GitRepoScope {
            repo: String::new(),
        }),
    )
    .await
    .expect("sync status");
    body_of(response).await
}

/// A committed repo plus a clone of it that the origin can be pushed to.
pub(crate) fn repo_with_clone() -> (TempDir, TempDir) {
    let origin = init_repo();
    commit(origin.path(), "a.txt", "one\n");
    // Pushing into a non-bare checkout is refused; allow it explicitly where
    // a test wants a push to succeed.
    run_git(origin.path(), &["config", "receive.denyCurrentBranch", "warn"]);

    let work = TempDir::new().expect("tempdir");
    let clone_dir = work.path().join("clone");
    let out = Command::new("git")
        .args(["clone", "-q"])
        .arg(origin.path())
        .arg(&clone_dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("clone");
    assert!(out.status.success(), "clone failed: {out:?}");
    run_git(&clone_dir, &["config", "user.name", "Test User"]);
    run_git(&clone_dir, &["config", "user.email", "test@example.com"]);
    (origin, work)
}

// ── git_sync_status ──────────────────────────────────────────────────

#[tokio::test]
async fn status_unborn_repo() {
    let td = init_repo();
    let body = status_of(td.path()).await;
    assert_eq!(body["unborn"], true);
    assert_eq!(body["branch"], "");
    assert_eq!(body["detached"], false);
    assert_eq!(body["ahead"], 0);
    assert_eq!(body["behind"], 0);
    assert!(body["remotes"].as_array().expect("remotes").is_empty());
}

#[tokio::test]
async fn status_branch_without_upstream() {
    let td = init_repo();
    commit(td.path(), "a.txt", "one\n");
    let body = status_of(td.path()).await;
    assert_eq!(body["unborn"], false);
    assert_eq!(body["branch"], "main");
    assert_eq!(body["detached"], false);
    assert!(body["upstream"].is_null());
}

#[tokio::test]
async fn status_detached_head_reports_short_hash() {
    let td = init_repo();
    commit(td.path(), "a.txt", "one\n");
    commit(td.path(), "b.txt", "two\n");
    let head = run_git(td.path(), &["rev-parse", "--short", "HEAD"]);
    let short = String::from_utf8_lossy(&head.stdout).trim().to_string();
    run_git(td.path(), &["checkout", "-q", "--detach", "HEAD"]);

    let body = status_of(td.path()).await;
    assert_eq!(body["detached"], true);
    assert_eq!(body["branch"], short);
}

#[tokio::test]
async fn status_counts_ahead_and_behind() {
    let (origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");

    // Two commits only on the origin → the clone is behind by two.
    commit(origin.path(), "b.txt", "two\n");
    commit(origin.path(), "c.txt", "three\n");
    run_git(&clone_dir, &["fetch", "-q", "origin"]);
    // One commit only on the clone → ahead by one.
    commit(&clone_dir, "local.txt", "local\n");

    let body = status_of(&clone_dir).await;
    assert_eq!(body["upstream"], "origin/main");
    assert_eq!(body["ahead"], 1);
    assert_eq!(body["behind"], 2);
    assert_eq!(body["branch"], "main");
}

#[tokio::test]
async fn status_parses_remotes_with_distinct_push_url() {
    let td = init_repo();
    commit(td.path(), "a.txt", "one\n");
    run_git(td.path(), &["remote", "add", "origin", "/tmp/fetch-side.git"]);
    run_git(
        td.path(),
        &["remote", "set-url", "--push", "origin", "/tmp/push-side.git"],
    );
    run_git(td.path(), &["remote", "add", "mirror", "/tmp/mirror.git"]);

    let body = status_of(td.path()).await;
    let remotes = body["remotes"].as_array().expect("remotes");
    assert_eq!(remotes.len(), 2);
    let origin = remotes
        .iter()
        .find(|r| r["name"] == "origin")
        .expect("origin");
    assert_eq!(origin["fetchUrl"], "/tmp/fetch-side.git");
    assert_eq!(origin["pushUrl"], "/tmp/push-side.git");
    let mirror = remotes
        .iter()
        .find(|r| r["name"] == "mirror")
        .expect("mirror");
    assert_eq!(mirror["fetchUrl"], "/tmp/mirror.git");
    // Same URL both ways → no separate push URL is reported.
    assert!(mirror["pushUrl"].is_null());
}

// ── git_fetch ────────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_from_local_clone_succeeds() {
    let (origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    commit(origin.path(), "b.txt", "two\n");

    let response = git_fetch(
        State(state_for(&clone_dir)),
        auth(),
        Json(GitFetchRequest {
            remote: Some("origin".to_string()),
            prune: true,
            repo: String::new(),
        }),
    )
    .await
    .expect("fetch");
    let body = body_of(response).await;
    assert_eq!(body["ok"], true);

    let body = status_of(&clone_dir).await;
    assert_eq!(body["behind"], 1);
}

#[tokio::test]
async fn fetch_rejects_an_option_shaped_remote() {
    let td = init_repo();
    let err = git_fetch(
        State(state_for(td.path())),
        auth(),
        Json(GitFetchRequest {
            remote: Some("--upload-pack=evil".to_string()),
            prune: false,
            repo: String::new(),
        }),
    )
    .await;
    assert!(err.is_err(), "an option-shaped remote must be rejected");
}

// ── git_push ─────────────────────────────────────────────────────────

pub(crate) fn push_request() -> GitPushRequest {
    GitPushRequest {
        remote: None,
        branch: None,
        set_upstream: false,
        force: false,
        repo: String::new(),
    }
}

// ── git_pull ─────────────────────────────────────────────────────────
