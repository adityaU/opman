//! Stash listing and the ref validation every action now shares.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;
use std::path::Path;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("spawn git")
}

fn repo() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("f.txt"), "one").expect("write");
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-q", "-m", "root"]);
    tmp
}

fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

#[tokio::test]
async fn list_pairs_each_entry_with_its_ref() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::write(dir.join("f.txt"), "two").expect("write");
    git(dir, &["stash", "push", "-m", "first"]);
    std::fs::write(dir.join("f.txt"), "three").expect("write");
    git(dir, &["stash", "push", "-m", "second"]);

    let response = list(dir).await.expect("list");
    assert_eq!(response.entries.len(), 2);
    assert_eq!(response.entries[0].reference, "stash@{0}");
    assert_eq!(response.entries[1].reference, "stash@{1}");
    assert!(response.entries[0].message.contains("second"));
    assert!(!response.entries[0].hash.is_empty());
}

#[tokio::test]
async fn list_of_an_empty_stash_is_empty_not_an_error() {
    let tmp = repo();
    let response = list(tmp.path()).await.expect("list");
    assert!(response.entries.is_empty());
    assert!(response.action.ok);
}

/// The old string-dispatch validated `pop`'s ref but not `drop`'s.
#[tokio::test]
async fn every_ref_taking_action_rejects_an_option_lookalike() {
    let tmp = repo();
    let state = state_for(tmp.path());

    for action in [
        GitStashAction::Pop,
        GitStashAction::Apply,
        GitStashAction::Drop,
    ] {
        let request = GitStashRequest {
            action,
            message: None,
            stash_ref: Some("--upload-pack=evil".to_string()),
            repo: String::new(),
        };
        let result = git_stash(
            axum::extract::State(state.clone()),
            AuthUser { subject: "test".into() },
            axum::response::Json(request),
        )
        .await;
        assert!(
            result.is_err(),
            "{action:?} must reject a ref that could be read as an option"
        );
    }
}

#[tokio::test]
async fn apply_keeps_the_entry_while_pop_removes_it() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::write(dir.join("f.txt"), "changed").expect("write");
    git(dir, &["stash", "push", "-m", "work"]);

    let state = state_for(dir);
    let apply = GitStashRequest {
        action: GitStashAction::Apply,
        message: None,
        stash_ref: None,
        repo: String::new(),
    };
    git_stash(
        axum::extract::State(state.clone()),
        AuthUser { subject: "test".into() },
        axum::response::Json(apply),
    )
    .await
    .expect("apply");

    assert_eq!(
        list(dir).await.expect("list").entries.len(),
        1,
        "apply should leave the entry in place"
    );

    git(dir, &["checkout", "--", "f.txt"]);
    let pop = GitStashRequest {
        action: GitStashAction::Pop,
        message: None,
        stash_ref: None,
        repo: String::new(),
    };
    git_stash(
        axum::extract::State(state),
        AuthUser { subject: "test".into() },
        axum::response::Json(pop),
    )
    .await
    .expect("pop");

    assert!(list(dir).await.expect("list").entries.is_empty());
}

/// Switching branches after a stash is the point of stashing, and untracked
/// files left behind would defeat it.
#[tokio::test]
async fn push_includes_untracked_files() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::write(dir.join("new.txt"), "untracked").expect("write");

    let state = state_for(dir);
    let request = GitStashRequest {
        action: GitStashAction::Push,
        message: Some("with untracked".to_string()),
        stash_ref: None,
        repo: String::new(),
    };
    git_stash(
        axum::extract::State(state),
        AuthUser { subject: "test".into() },
        axum::response::Json(request),
    )
    .await
    .expect("push");

    assert!(
        !dir.join("new.txt").exists(),
        "the untracked file should have been stashed"
    );
}

#[test]
fn parse_entry_skips_blank_rows_and_splits_on_tabs() {
    assert!(parse_entry(0, "").is_none());
    let entry = parse_entry(3, "stash@{3}\tWIP on main\t2 hours ago\tabc1234")
        .expect("row parses");
    assert_eq!(entry.index, 3);
    assert_eq!(entry.reference, "stash@{3}");
    assert_eq!(entry.message, "WIP on main");
    assert_eq!(entry.age, "2 hours ago");
    assert_eq!(entry.hash, "abc1234");
}

#[test]
fn every_action_maps_to_a_git_verb() {
    assert_eq!(GitStashAction::default(), GitStashAction::Push);
    for (action, verb) in [
        (GitStashAction::Push, "push"),
        (GitStashAction::Pop, "pop"),
        (GitStashAction::Apply, "apply"),
        (GitStashAction::Drop, "drop"),
        (GitStashAction::List, "list"),
    ] {
        assert_eq!(action.verb(), verb);
    }
}
