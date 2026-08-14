//! Merge and rebase, separated from the in-flight operation probing.

use super::git_integrate_tests::*;
use super::super::git_integrate::*;
use axum::extract::State;
use axum::response::Json;

#[tokio::test]
async fn fast_forward_merge_moves_the_branch_without_a_commit() {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "a.txt", "a\n", "init");
    run_git(dir, &["checkout", "-q", "-b", "side"]);
    commit(dir, "b.txt", "b\n", "ahead");
    let tip = git_out(dir, &["rev-parse", "HEAD"]);
    run_git(dir, &["checkout", "-q", "main"]);

    let body = merge(&state_for(dir), "side", false).await;
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!(git_out(dir, &["rev-parse", "HEAD"]), tip, "fast-forwarded");
    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "2");
}

#[tokio::test]
async fn no_ff_merge_records_a_merge_commit() {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "a.txt", "a\n", "init");
    run_git(dir, &["checkout", "-q", "-b", "side"]);
    commit(dir, "b.txt", "b\n", "ahead");
    run_git(dir, &["checkout", "-q", "main"]);

    let body = merge(&state_for(dir), "side", true).await;
    assert_eq!(body["ok"], true, "{body}");
    let parents = git_out(dir, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(parents.split(' ').count(), 3, "merge commit: {parents}");
}

#[tokio::test]
async fn merge_rejects_an_option_shaped_branch() {
    let td = init_repo();
    commit(td.path(), "a.txt", "a\n", "init");
    let state = state_for(td.path());
    let req = GitMergeRequest {
        branch: "--exec=touch pwned".to_string(),
        no_ff: false,
        no_commit: false,
        repo: String::new(),
    };
    let err = git_merge(State(state), auth(), Json(req)).await;
    assert!(err.is_err(), "argv injection must not reach git");
}

#[tokio::test]
async fn rebase_replays_the_branch_and_reports_progress_on_conflict() {
    let td = diverged_repo();
    let dir = td.path();
    let state = state_for(dir);
    run_git(dir, &["checkout", "-q", "side"]);

    let req = GitRebaseRequest {
        onto: "main".to_string(),
        repo: String::new(),
    };
    let response = git_rebase(State(state.clone()), auth(), Json(req))
        .await
        .expect("rebase handler");
    let body = body_of(response).await;
    assert_eq!(body["ok"], false, "the replay conflicts: {body}");

    let status = status_of(&state, "").await;
    assert_eq!(status["kind"], "rebase");
    assert_eq!(status["step"], 1);
    assert_eq!(status["total"], 1);
    assert_eq!(status["conflicted"], serde_json::json!(["f.txt"]));

    let aborted = act(&state, GitOperationAction::Abort).await;
    assert_eq!(aborted["ok"], true, "{aborted}");
    assert!(status_of(&state, "").await.get("kind").is_none());
}

#[tokio::test]
async fn clean_rebase_succeeds() {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "a.txt", "a\n", "init");
    run_git(dir, &["checkout", "-q", "-b", "side"]);
    commit(dir, "s.txt", "s\n", "side only");
    run_git(dir, &["checkout", "-q", "main"]);
    commit(dir, "m.txt", "m\n", "main only");
    run_git(dir, &["checkout", "-q", "side"]);

    let req = GitRebaseRequest {
        onto: "main".to_string(),
        repo: String::new(),
    };
    let response = git_rebase(State(state_for(dir)), auth(), Json(req))
        .await
        .expect("rebase handler");
    let body = body_of(response).await;
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "3");
}
