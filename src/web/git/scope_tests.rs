//! Repo-scope resolution, including the linked-worktree case.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;

fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

fn init_repo(dir: &Path) {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@example.com"][..],
        &["config", "user.name", "Test"][..],
        &["commit", "-q", "--allow-empty", "-m", "root"][..],
    ] {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("git setup");
    }
}

#[tokio::test]
async fn dot_and_empty_resolve_to_the_project_root() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let state = state_for(tmp.path());

    for scope in ["", "."] {
        let resolved = resolve(&state, scope).await.expect("resolves");
        assert_eq!(resolved.path(), tmp.path());
    }
}

#[tokio::test]
async fn missing_project_is_a_bad_request() {
    let state = test_server_state();
    assert!(matches!(
        resolve(&state, ".").await,
        Err(WebError::BadRequest(_))
    ));
}

#[tokio::test]
async fn nested_repository_resolves() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let nested = tmp.path().join("sub");
    std::fs::create_dir_all(&nested).expect("mkdir");
    init_repo(&nested);

    let state = state_for(tmp.path());
    let resolved = resolve(&state, "sub").await.expect("resolves");
    assert!(resolved.path().ends_with("sub"));
}

#[tokio::test]
async fn non_repository_directory_is_rejected() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("plain")).expect("mkdir");

    let state = state_for(tmp.path());
    assert!(matches!(
        resolve(&state, "plain").await,
        Err(WebError::BadRequest(_))
    ));
}

#[tokio::test]
async fn escaping_the_project_is_rejected() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("mkdir");
    init_repo(&tmp.path().join("project"));

    let state = state_for(&project);
    assert!(matches!(
        resolve(&state, "..").await,
        Err(WebError::BadRequest(_))
    ));
}

#[tokio::test]
async fn missing_path_is_not_found() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let state = state_for(tmp.path());
    assert!(matches!(
        resolve(&state, "nope").await,
        Err(WebError::NotFound(_))
    ));
}

/// A linked worktree stores `.git` as a file pointing at the main repository.
/// Resolution must accept that, or every worktree is invisible to the panel.
#[tokio::test]
async fn linked_worktree_resolves_despite_dot_git_being_a_file() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let main = tmp.path().join("main");
    std::fs::create_dir_all(&main).expect("mkdir");
    init_repo(&main);

    let linked = tmp.path().join("wt");
    std::process::Command::new("git")
        .args(["worktree", "add", "-q", "-b", "side"])
        .arg(&linked)
        .current_dir(&main)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("worktree add");

    assert!(linked.join(".git").is_file(), "worktree .git should be a file");

    let state = state_for(tmp.path());
    let resolved = resolve(&state, "wt").await.expect("worktree resolves");
    assert!(resolved.path().ends_with("wt"));
}
