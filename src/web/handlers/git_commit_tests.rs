//! Committing, amending and staging everything in one step.

use super::super::git_handlers::git_handlers_tests::{call, commit_all, init_repo, run_git, state_for, write_file};
use axum::http::StatusCode;

#[tokio::test]
async fn commit_stage_all_picks_up_tracked_edits() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    write_file(dir, "a.txt", "2\n");
    let state = state_for(dir);

    let (status, body) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "edit", "stageAll": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);

    let porcelain = run_git(dir, &["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
        "the edit should have been staged and committed"
    );
}

/// an option and there is no reason to refuse a subject like "-oops".
#[tokio::test]
async fn commit_message_may_begin_with_a_dash() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    run_git(dir, &["add", "-A"]);
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "-oops"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);

    let subject = run_git(dir, &["log", "-1", "--format=%s"]);
    assert_eq!(String::from_utf8_lossy(&subject.stdout).trim(), "-oops");
}

/// error — a 500 would reduce git's explanation to a generic toast.
#[tokio::test]
async fn commit_nothing_staged_is_a_refusal_not_an_error() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "init");
    let state = state_for(dir);
    let (status, body) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "nothing"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], false);
    assert!(body.get("hash").is_none(), "no commit was created");
    assert!(
        body["message"].as_str().is_some_and(|m| !m.is_empty()),
        "git's own reason must survive"
    );
}

/// Nothing staged is a refusal the panel renders inline, not a transport
#[tokio::test]
async fn commit_amend_replaces_the_previous_commit() {
    let td = init_repo();
    let dir = td.path();
    write_file(dir, "a.txt", "1\n");
    commit_all(dir, "original");
    let state = state_for(dir);

    let (status, body) = call(
        &state,
        "POST",
        "/api/git/commit",
        Some(serde_json::json!({"message": "corrected", "amend": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);

    let subject = run_git(dir, &["log", "-1", "--format=%s"]);
    assert_eq!(String::from_utf8_lossy(&subject.stdout).trim(), "corrected");
    let count = run_git(dir, &["rev-list", "--count", "HEAD"]);
    assert_eq!(
        String::from_utf8_lossy(&count.stdout).trim(),
        "1",
        "amend replaces rather than adds"
    );
}

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

/// A message is one argv token after `-m`, so a leading dash cannot be read as
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
    assert_eq!(body["ok"], true);
    assert!(!body["hash"].as_str().unwrap_or_default().is_empty());
}
