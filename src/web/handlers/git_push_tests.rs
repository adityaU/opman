//! Push and pull, the two endpoints that can be rejected by a remote.

use super::git_sync_tests::*;
use super::super::git_sync::*;
use axum::extract::State;
use axum::response::Json;

#[tokio::test]
async fn push_uses_upstream_remote_by_default() {
    let (_origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    commit(&clone_dir, "local.txt", "local\n");

    let response = git_push(State(state_for(&clone_dir)), auth(), Json(push_request()))
        .await
        .expect("push");
    let body = body_of(response).await;
    assert_eq!(body["ok"], true, "push body: {body}");
}

#[tokio::test]
async fn push_reports_a_rejection_as_a_refusal() {
    let (origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    // Both sides move → the remote is ahead, so a plain push is rejected.
    commit(origin.path(), "server.txt", "server\n");
    commit(&clone_dir, "local.txt", "local\n");

    let response = git_push(State(state_for(&clone_dir)), auth(), Json(push_request()))
        .await
        .expect("push");
    let body = body_of(response).await;
    assert_eq!(body["ok"], false, "push body: {body}");
    assert!(body["failure"].is_string());
    assert!(body["hint"].is_string());
}

#[tokio::test]
async fn push_refuses_cleanly_when_head_is_detached() {
    let (_origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    run_git(&clone_dir, &["checkout", "-q", "--detach", "HEAD"]);

    let response = git_push(State(state_for(&clone_dir)), auth(), Json(push_request()))
        .await
        .expect("push");
    let body = body_of(response).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["failure"], "not_found");
    assert!(body["message"]
        .as_str()
        .expect("message")
        .contains("not on a branch"));
}

#[tokio::test]
async fn push_blocks_when_no_remote_is_configured() {
    let td = init_repo();
    commit(td.path(), "a.txt", "one\n");
    let response = git_push(State(state_for(td.path())), auth(), Json(push_request()))
        .await
        .expect("push");
    let body = body_of(response).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["message"], "No remote configured");
}

#[tokio::test]
async fn pull_fast_forwards_from_the_clone_origin() {
    let (origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    commit(origin.path(), "b.txt", "two\n");

    let response = git_pull(
        State(state_for(&clone_dir)),
        auth(),
        Json(GitPullRequest {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            repo: String::new(),
        }),
    )
    .await
    .expect("pull");
    let body = body_of(response).await;
    assert_eq!(body["ok"], true, "pull body: {body}");
    assert!(clone_dir.join("b.txt").exists());
}

#[tokio::test]
async fn pull_refuses_when_a_fast_forward_is_impossible() {
    let (origin, work) = repo_with_clone();
    let clone_dir = work.path().join("clone");
    commit(origin.path(), "server.txt", "server\n");
    commit(&clone_dir, "local.txt", "local\n");

    let response = git_pull(
        State(state_for(&clone_dir)),
        auth(),
        Json(GitPullRequest {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            repo: String::new(),
        }),
    )
    .await
    .expect("pull");
    let body = body_of(response).await;
    assert_eq!(body["ok"], false, "pull body: {body}");
    // Diverged, never a silent merge commit.
    assert!(!clone_dir.join("server.txt").exists());
}
