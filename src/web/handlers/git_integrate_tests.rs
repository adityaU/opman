//! Tests for git_integrate.rs, driven against real temporary repositories.
//!
//! Nothing here is mocked: every assertion is about what git actually did to a
//! repository on disk, which is the only way to be sure the operation-state
//! probing matches git's own bookkeeping.

use super::*;
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// ── repository fixtures ──────────────────────────────────────────────

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

pub(crate) fn git_out(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&run_git(dir, args).stdout)
        .trim()
        .to_string()
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

pub(crate) fn commit(dir: &Path, name: &str, content: &str, msg: &str) {
    write_file(dir, name, content);
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", msg]);
}

/// A repo whose `main` and `side` branches both changed `f.txt`, so merging
/// them conflicts.
pub(crate) fn diverged_repo() -> TempDir {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "f.txt", "base\n", "init");
    run_git(dir, &["checkout", "-q", "-b", "side"]);
    commit(dir, "f.txt", "side\n", "side change");
    run_git(dir, &["checkout", "-q", "main"]);
    commit(dir, "f.txt", "main\n", "main change");
    td
}

pub(crate) fn state_for(dir: &Path) -> ServerState {
    let mut state = crate::web::test_support::test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

pub(crate) fn auth() -> AuthUser {
    AuthUser {
        subject: "test".to_string(),
    }
}

// ── calling the handlers ─────────────────────────────────────────────

pub(crate) async fn body_of(response: impl IntoResponse) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_response().into_body(), 1 << 20)
        .await
        .expect("collect body");
    serde_json::from_slice(&bytes).expect("json body")
}

pub(crate) async fn status_of(state: &ServerState, repo: &str) -> serde_json::Value {
    let query = Query(GitRepoScope {
        repo: repo.to_string(),
    });
    let response = git_operation_status(State(state.clone()), auth(), query)
        .await
        .expect("status handler");
    body_of(response).await
}

pub(crate) async fn act(state: &ServerState, action: GitOperationAction) -> serde_json::Value {
    let req = GitOperationRequest {
        action,
        repo: String::new(),
    };
    let response = git_operation(State(state.clone()), auth(), Json(req))
        .await
        .expect("operation handler");
    body_of(response).await
}

pub(crate) async fn merge(state: &ServerState, branch: &str, no_ff: bool) -> serde_json::Value {
    let req = GitMergeRequest {
        branch: branch.to_string(),
        no_ff,
        no_commit: false,
        repo: String::new(),
    };
    let response = git_merge(State(state.clone()), auth(), Json(req))
        .await
        .expect("merge handler");
    body_of(response).await
}

pub(crate) async fn reset(state: &ServerState, target: &str, mode: GitResetMode) -> serde_json::Value {
    let req = GitResetRequest {
        target: target.to_string(),
        mode,
        repo: String::new(),
    };
    let response = git_reset(State(state.clone()), auth(), Json(req))
        .await
        .expect("reset handler");
    body_of(response).await
}

pub(crate) async fn replay(state: &ServerState, hash: &str, revert: bool) -> serde_json::Value {
    let req = GitReplayRequest {
        hash: hash.to_string(),
        no_commit: false,
        repo: String::new(),
    };
    let response = if revert {
        git_revert(State(state.clone()), auth(), Json(req))
            .await
            .expect("revert handler")
            .into_response()
    } else {
        git_cherry_pick(State(state.clone()), auth(), Json(req))
            .await
            .expect("cherry-pick handler")
            .into_response()
    };
    body_of(response).await
}

// ── operation status ─────────────────────────────────────────────────

#[tokio::test]
async fn clean_repo_reports_no_operation() {
    let td = init_repo();
    commit(td.path(), "a.txt", "a\n", "init");
    let body = status_of(&state_for(td.path()), "").await;

    assert!(body.get("kind").is_none(), "clean repo has no operation");
    assert_eq!(body["conflicted"].as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn conflicting_merge_reports_merge_and_its_files() {
    let td = diverged_repo();
    let state = state_for(td.path());

    let merged = merge(&state, "side", false).await;
    assert_eq!(merged["ok"], false, "the merge conflicts");
    assert_eq!(merged["failure"], "conflict");

    let body = status_of(&state, "").await;
    assert_eq!(body["kind"], "merge");
    assert_eq!(body["conflicted"], serde_json::json!(["f.txt"]));
}

#[tokio::test]
async fn abort_clears_the_operation() {
    let td = diverged_repo();
    let state = state_for(td.path());
    merge(&state, "side", false).await;

    let aborted = act(&state, GitOperationAction::Abort).await;
    assert_eq!(aborted["ok"], true, "{aborted}");

    let body = status_of(&state, "").await;
    assert!(body.get("kind").is_none());
    assert_eq!(body["conflicted"].as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn resolving_then_continuing_lands_a_merge_commit() {
    let td = diverged_repo();
    let dir = td.path();
    let state = state_for(dir);
    merge(&state, "side", false).await;

    write_file(dir, "f.txt", "resolved\n");
    run_git(dir, &["add", "f.txt"]);
    let done = act(&state, GitOperationAction::Continue).await;
    assert_eq!(done["ok"], true, "{done}");

    // Two parents means git recorded a real merge rather than a plain commit.
    assert_eq!(git_out(dir, &["rev-list", "--parents", "-n", "1", "HEAD"]).split(' ').count(), 3);
    assert!(status_of(&state, "").await.get("kind").is_none());
}

#[tokio::test]
async fn operation_detection_works_inside_a_linked_worktree() {
    let td = diverged_repo();
    let dir = td.path();
    // `main` is checked out in the primary worktree, so the linked one takes a
    // detached HEAD at main and merges side there.
    let linked = dir.join("wt");
    run_git(
        dir,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().expect("utf-8 path"),
            "main",
        ],
    );
    assert!(linked.join(".git").is_file(), ".git is a file here");

    let state = state_for(&linked);
    let merged = merge(&state, "side", false).await;
    assert_eq!(merged["ok"], false);

    // This is the whole point of `rev-parse --git-path`: the marker lives in
    // .git/worktrees/wt/MERGE_HEAD, not in <linked>/.git/MERGE_HEAD.
    let body = status_of(&state, "").await;
    assert_eq!(body["kind"], "merge");
    assert_eq!(body["conflicted"], serde_json::json!(["f.txt"]));
}

// ── action matrix refusals ───────────────────────────────────────────

// ── merge shapes ─────────────────────────────────────────────────────

// ── rebase ───────────────────────────────────────────────────────────

// ── reset ────────────────────────────────────────────────────────────

/// A repo with two commits; the second adds `b.txt`.
pub(crate) fn two_commits() -> TempDir {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "a.txt", "a\n", "init");
    commit(dir, "b.txt", "b\n", "second");
    td
}

// ── revert and cherry-pick ───────────────────────────────────────────
