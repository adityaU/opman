//! Creating a worktree, and the path containment that guards it.

use super::git_worktree_tests::*;
use super::super::git_worktree::*;
use axum::extract::State;
use axum::response::Json;

#[tokio::test]
async fn add_list_remove_prune_round_trip() {
    let td = init_repo();
    let state = state_for(td.path());

    let (status, body) = parts(
        git_worktree_add(
            State(state.clone()),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "trees/feat".into(),
                branch: "feat".into(),
                create: true,
                start_point: None,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["ok"], true, "add failed: {body}");

    let body = list(&state).await;
    let trees = body["worktrees"].as_array().expect("array");
    assert_eq!(trees.len(), 2);
    let added = trees
        .iter()
        .find(|t| t["branch"] == "feat")
        .expect("added worktree");
    assert_eq!(added["relative"], "trees/feat");
    assert_eq!(added["main"], false);
    assert_eq!(added["current"], false);
    assert_eq!(added["locked"], false);

    let (_, body) = parts(
        git_worktree_remove(
            State(state.clone()),
            auth(),
            Json(GitWorktreeRemoveRequest {
                path: "trees/feat".into(),
                force: false,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(body["ok"], true, "remove failed: {body}");

    let body = list(&state).await;
    assert_eq!(body["worktrees"].as_array().expect("array").len(), 1);

    let (status, body) = parts(
        git_worktree_prune(
            State(state.clone()),
            auth(),
            Json(GitRepoScope::default()),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn add_checks_out_an_existing_branch_without_create() {
    let td = init_repo();
    run_git(td.path(), &["branch", "existing"]);
    let state = state_for(td.path());

    let (_, body) = parts(
        git_worktree_add(
            State(state.clone()),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "wt".into(),
                branch: "existing".into(),
                create: false,
                start_point: None,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(body["ok"], true, "add failed: {body}");
    let body = list(&state).await;
    assert!(body["worktrees"]
        .as_array()
        .expect("array")
        .iter()
        .any(|t| t["branch"] == "existing"));
}

#[tokio::test]
async fn add_honours_an_explicit_start_point() {
    let td = init_repo();
    let state = state_for(td.path());
    let (_, body) = parts(
        git_worktree_add(
            State(state.clone()),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "wt".into(),
                branch: "from-main".into(),
                create: true,
                start_point: Some("main".into()),
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(body["ok"], true, "add failed: {body}");
}

#[tokio::test]
async fn add_refuses_a_branch_name_that_looks_like_an_option() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, _) = parts(
        git_worktree_add(
            State(state),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "wt".into(),
                branch: "--force".into(),
                create: true,
                start_point: None,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_refuses_a_path_escaping_the_project() {
    let td = init_repo();
    let state = state_for(td.path());
    let (status, _) = parts(
        git_worktree_add(
            State(state),
            auth(),
            Json(GitWorktreeAddRequest {
                path: "../outside".into(),
                branch: "feat".into(),
                create: true,
                start_point: None,
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}
