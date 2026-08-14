//! Branch switching against real repositories.
//!
//! The remote-tracking cases are the point: a plain `git checkout origin/x`
//! succeeds and detaches HEAD, so a test that only asserts "the request
//! succeeded" would have passed against the broken behaviour too. Every case
//! here asserts where HEAD actually ended up.

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
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("spawn git")
}

fn init(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "root"]);
}

fn head_of(dir: &Path) -> String {
    let out = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

/// A repo cloned from an origin, so remote-tracking refs are real.
fn cloned() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let origin = tmp.path().join("origin");
    std::fs::create_dir_all(&origin).expect("mkdir");
    init(&origin);
    git(&origin, &["checkout", "-q", "-b", "feature/login"]);
    git(&origin, &["commit", "-q", "--allow-empty", "-m", "feature work"]);
    git(&origin, &["checkout", "-q", "main"]);

    let clone = tmp.path().join("clone");
    git(
        tmp.path(),
        &[
            "clone",
            "-q",
            &origin.to_string_lossy(),
            &clone.to_string_lossy(),
        ],
    );
    git(&clone, &["config", "user.email", "t@example.com"]);
    git(&clone, &["config", "user.name", "Test"]);
    (tmp, clone)
}

async fn checkout(dir: &Path, branch: &str, carry: bool) -> GitActionResponse {
    let state = state_for(dir);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let remotes = remote_names(repo.path()).await.expect("remotes");
    let requested = RefName::parse(branch).expect("valid ref");
    let target = Target::resolve(repo.path(), requested, &remotes)
        .await
        .expect("resolve");

    let mut args: Vec<&str> = vec!["checkout"];
    if carry {
        args.push("--merge");
    }
    match target {
        Target::Existing(name) => args.push(name.as_str()),
        Target::Track { local, remote } => {
            args.extend_from_slice(&["-b", local, "--track", remote.as_str()]);
        }
    }
    GitActionResponse::from(
        exec::run(repo.path(), &args, Reach::Local)
            .await
            .expect("spawned"),
    )
}

/// The original defect: this used to leave HEAD detached and report success.
#[tokio::test]
async fn checking_out_a_remote_branch_creates_a_tracking_branch() {
    let (_tmp, clone) = cloned();

    let response = checkout(&clone, "origin/feature/login", false).await;
    assert!(response.ok, "checkout should succeed: {}", response.message);

    assert_eq!(
        head_of(&clone),
        "feature/login",
        "HEAD must be on a local branch, not detached at the remote ref"
    );

    let upstream = git(
        &clone,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
    );
    assert_eq!(
        String::from_utf8_lossy(&upstream.stdout).trim(),
        "origin/feature/login",
        "the new branch should track the remote it came from"
    );
}

#[tokio::test]
async fn a_remote_name_with_an_existing_local_branch_switches_to_the_local_one() {
    let (_tmp, clone) = cloned();
    git(&clone, &["checkout", "-q", "-b", "feature/login", "origin/feature/login"]);
    git(&clone, &["checkout", "-q", "main"]);

    let response = checkout(&clone, "origin/feature/login", false).await;
    assert!(response.ok, "{}", response.message);
    assert_eq!(head_of(&clone), "feature/login");

    // No duplicate was created.
    let branches = git(&clone, &["branch", "--format=%(refname:short)"]);
    let names: Vec<_> = String::from_utf8_lossy(&branches.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    assert_eq!(names.iter().filter(|n| *n == "feature/login").count(), 1);
}

#[tokio::test]
async fn a_local_branch_whose_name_looks_remote_is_not_split() {
    let (_tmp, clone) = cloned();
    // `origin` is a remote, but `originals/x` is not a remote-tracking name.
    git(&clone, &["branch", "originals/x"]);

    let response = checkout(&clone, "originals/x", false).await;
    assert!(response.ok, "{}", response.message);
    assert_eq!(head_of(&clone), "originals/x");
}

#[tokio::test]
async fn a_dirty_tree_refuses_with_a_recovery_rather_than_silence() {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    init(dir);
    std::fs::write(dir.join("f.txt"), "one").expect("write");
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-q", "-m", "add f"]);
    git(dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("f.txt"), "two").expect("write");
    git(dir, &["commit", "-qam", "change f"]);
    git(dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("f.txt"), "uncommitted").expect("write");

    let response = checkout(dir, "other", false).await;
    assert!(!response.ok);
    assert_eq!(response.failure, Some(GitFailure::DirtyTree));
    assert!(
        response.hint.is_some_and(|h| h.contains("stash")),
        "the refusal must name a recovery"
    );
    assert_eq!(head_of(dir), "main", "the failed switch must not move HEAD");
}

#[tokio::test]
async fn carrying_changes_across_succeeds_where_a_plain_switch_refuses() {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    init(dir);
    std::fs::write(dir.join("a.txt"), "a").expect("write");
    git(dir, &["add", "a.txt"]);
    git(dir, &["commit", "-q", "-m", "add a"]);
    git(dir, &["branch", "other"]);
    // A change to a file that `other` does not touch carries cleanly.
    std::fs::write(dir.join("b.txt"), "b").expect("write");

    let response = checkout(dir, "other", true).await;
    assert!(response.ok, "{}", response.message);
    assert_eq!(head_of(dir), "other");
    assert!(dir.join("b.txt").exists(), "the change should come along");
}

#[tokio::test]
async fn a_missing_branch_is_reported_as_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    init(tmp.path());

    let response = checkout(tmp.path(), "nope", false).await;
    assert!(!response.ok);
    assert_eq!(response.failure, Some(GitFailure::NotFound));
}

#[tokio::test]
async fn branch_listing_reports_tracking_and_the_current_branch() {
    let (_tmp, clone) = cloned();
    let state = state_for(&clone);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");
    let (local, remote) = collect(repo.path(), &head).await.expect("collect");

    let main = local.iter().find(|b| b.name == "main").expect("main listed");
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
        &["worktree", "add", "-q", "-b", "side", &linked.to_string_lossy()],
    );

    let state = state_for(&main);
    let repo = scope::resolve(&state, "").await.expect("scope");
    let head = current_head(repo.path()).await.expect("head");
    let (local, _) = collect(repo.path(), &head).await.expect("collect");

    let side = local.iter().find(|b| b.name == "side").expect("side listed");
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
