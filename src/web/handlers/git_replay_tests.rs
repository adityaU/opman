//! Reset, revert and cherry-pick: the commit-replay half of git_integrate.

use super::git_integrate_tests::*;
use super::super::git_integrate::*;
use axum::extract::State;
use axum::response::Json;

#[tokio::test]
async fn reset_soft_keeps_the_change_staged() {
    let td = two_commits();
    let dir = td.path();
    let body = reset(&state_for(dir), "HEAD~1", GitResetMode::Soft).await;
    assert_eq!(body["ok"], true, "{body}");

    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(git_out(dir, &["diff", "--cached", "--name-only"]), "b.txt");
    assert!(dir.join("b.txt").exists());
}

#[tokio::test]
async fn reset_mixed_unstages_but_keeps_the_file() {
    let td = two_commits();
    let dir = td.path();
    let body = reset(&state_for(dir), "HEAD~1", GitResetMode::Mixed).await;
    assert_eq!(body["ok"], true, "{body}");

    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(git_out(dir, &["diff", "--cached", "--name-only"]), "");
    assert_eq!(git_out(dir, &["status", "--porcelain"]), "?? b.txt");
}

#[tokio::test]
async fn reset_hard_destroys_the_change() {
    let td = two_commits();
    let dir = td.path();
    let body = reset(&state_for(dir), "HEAD~1", GitResetMode::Hard).await;
    assert_eq!(body["ok"], true, "{body}");

    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "1");
    assert!(!dir.join("b.txt").exists(), "hard reset removes the file");
    assert_eq!(git_out(dir, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn reset_accepts_a_bare_hash_and_rejects_junk() {
    let td = two_commits();
    let dir = td.path();
    let first = git_out(dir, &["rev-parse", "HEAD~1"]);

    let body = reset(&state_for(dir), &first, GitResetMode::Hard).await;
    assert_eq!(body["ok"], true, "{body}");
    assert_eq!(git_out(dir, &["rev-parse", "HEAD"]), first);

    let req = GitResetRequest {
        target: "--hard=/etc".to_string(),
        mode: GitResetMode::Soft,
        repo: String::new(),
    };
    assert!(git_reset(State(state_for(dir)), auth(), Json(req))
        .await
        .is_err());
}

#[tokio::test]
async fn revert_undoes_a_commit_without_opening_an_editor() {
    let td = two_commits();
    let dir = td.path();
    let head = git_out(dir, &["rev-parse", "HEAD"]);

    let body = replay(&state_for(dir), &head, true).await;
    assert_eq!(body["ok"], true, "{body}");
    assert!(!dir.join("b.txt").exists(), "the addition was reverted");
    assert_eq!(git_out(dir, &["rev-list", "--count", "HEAD"]), "3");
    assert!(git_out(dir, &["log", "-1", "--format=%s"]).starts_with("Revert"));
}

#[tokio::test]
async fn cherry_pick_applies_a_commit_from_another_branch() {
    let td = init_repo();
    let dir = td.path();
    commit(dir, "a.txt", "a\n", "init");
    run_git(dir, &["checkout", "-q", "-b", "side"]);
    commit(dir, "s.txt", "s\n", "side only");
    let picked = git_out(dir, &["rev-parse", "HEAD"]);
    run_git(dir, &["checkout", "-q", "main"]);

    let body = replay(&state_for(dir), &picked, false).await;
    assert_eq!(body["ok"], true, "{body}");
    assert!(dir.join("s.txt").exists());
    assert_eq!(git_out(dir, &["log", "-1", "--format=%s"]), "side only");
}

#[tokio::test]
async fn conflicting_cherry_pick_reports_its_own_kind() {
    let td = diverged_repo();
    let dir = td.path();
    let picked = git_out(dir, &["rev-parse", "side"]);
    let state = state_for(dir);

    let body = replay(&state, &picked, false).await;
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(status_of(&state, "").await["kind"], "cherry_pick");

    let aborted = act(&state, GitOperationAction::Abort).await;
    assert_eq!(aborted["ok"], true, "{aborted}");
    assert!(status_of(&state, "").await.get("kind").is_none());
}

#[tokio::test]
async fn replay_rejects_a_non_hash() {
    let td = two_commits();
    let state = state_for(td.path());
    let req = GitReplayRequest {
        hash: "--no-verify".to_string(),
        no_commit: false,
        repo: String::new(),
    };
    assert!(git_revert(State(state), auth(), Json(req)).await.is_err());
}
