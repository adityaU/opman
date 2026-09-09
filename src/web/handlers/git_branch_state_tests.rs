//! Branch listing and repository state tests.

use super::*;

#[tokio::test]
async fn branch_listing_reports_tracking_and_the_current_branch() {
    let (_tmp, clone) = cloned();
    let state = state_for(&clone);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");
    let (local, remote) = collect(repo.path(), &head).await.expect("collect");

    let main = local
        .iter()
        .find(|b| b.name == "main")
        .expect("main listed");
    assert!(main.current);
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));

    assert!(
        remote.iter().any(|b| b.name == "origin/feature/login"),
        "remote branches should be listed"
    );
    assert!(
        !remote.iter().any(|b| b.name.ends_with("/HEAD")),
        "the symbolic remote HEAD is not a branch"
    );
}

#[tokio::test]
async fn a_branch_checked_out_in_another_worktree_is_reported() {
    let tmp = TempDir::new().expect("tempdir");
    let main = tmp.path().join("main");
    std::fs::create_dir_all(&main).expect("mkdir");
    init(&main);
    let linked = tmp.path().join("wt");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            &linked.to_string_lossy(),
        ],
    );

    let state = state_for(&main);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");
    let (local, _) = collect(repo.path(), &head).await.expect("collect");

    let side = local
        .iter()
        .find(|b| b.name == "side")
        .expect("side listed");
    assert!(
        side.worktree.is_some(),
        "a branch held by another worktree must say so, since it cannot be checked out here"
    );
}

#[tokio::test]
async fn detached_head_is_reported_as_detached() {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    init(dir);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "second"]);
    git(dir, &["checkout", "-q", "HEAD~1"]);

    let state = state_for(dir);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");

    assert!(matches!(head, HeadState::Detached { .. }));
    assert_eq!(head.branch(), None);
}

#[tokio::test]
async fn an_unborn_repository_reports_unborn_rather_than_failing() {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q", "-b", "main"]);

    let state = state_for(tmp.path());
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");
    assert!(matches!(head, HeadState::Unborn));

    let (local, remote) = collect(repo.path(), &head).await.expect("collect");
    assert!(local.is_empty() && remote.is_empty());
}
