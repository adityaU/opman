//! Tests for `git_refs.rs` — tag listing/create/delete and blame, against real
//! temporary repositories.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::web_state::WebStateHandle;
use axum::response::IntoResponse;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

pub(crate) fn commit_as(dir: &Path, name: &str, email: &str, file: &str, body: &str, msg: &str) {
    std::fs::write(dir.join(file), body).expect("write file");
    run_git(dir, &["add", "-A"]);
    run_git(
        dir,
        &[
            "-c",
            &format!("user.name={name}"),
            "-c",
            &format!("user.email={email}"),
            "commit",
            "-q",
            "-m",
            msg,
        ],
    );
}

pub(crate) fn state_for(dir: &Path) -> ServerState {
    let mut state = test_server_state();
    state.web_state =
        WebStateHandle::new_test_with_projects(vec![("repo".to_string(), dir.to_path_buf())]);
    state
}

pub(crate) fn auth() -> AuthUser {
    AuthUser {
        subject: "t".into(),
    }
}

pub(crate) async fn body_json<T: IntoResponse>(r: WebResult<T>) -> serde_json::Value {
    let resp = r.expect("handler error").into_response();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

pub(crate) async fn tags(state: &ServerState) -> serde_json::Value {
    body_json(
        git_tags(
            State(state.clone()),
            auth(),
            Query(GitRepoScope::default()),
        )
        .await,
    )
    .await
}

pub(crate) async fn create(state: &ServerState, req: GitTagRequest) -> serde_json::Value {
    body_json(git_tag_create(State(state.clone()), auth(), Json(req)).await).await
}

pub(crate) fn tag_req(name: &str, message: Option<&str>, target: Option<&str>) -> GitTagRequest {
    GitTagRequest {
        name: name.into(),
        message: message.map(Into::into),
        target: target.map(Into::into),
        repo: String::new(),
    }
}

pub(crate) async fn blame(state: &ServerState, file: &str) -> WebResult<impl IntoResponse> {
    git_blame(
        State(state.clone()),
        auth(),
        Query(GitBlameQuery {
            file: file.into(),
            repo: String::new(),
        }),
    )
    .await
}

// ── tags ────────────────────────────────────────────────────────────

#[tokio::test]
async fn lists_lightweight_and_annotated_tags() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "first commit");
    run_git(dir, &["tag", "v0.1"]);
    run_git(dir, &["tag", "-a", "v0.2", "-m", "annotated subject"]);
    let state = state_for(dir);

    let body = tags(&state).await;
    let list = body["tags"].as_array().expect("tags array");
    assert_eq!(list.len(), 2);
    let by_name = |n: &str| {
        list.iter()
            .find(|t| t["name"] == n)
            .expect("tag present")
            .clone()
    };
    // A lightweight tag falls back to the commit subject.
    assert_eq!(by_name("v0.1")["subject"], "first commit");
    assert_eq!(by_name("v0.2")["subject"], "annotated subject");
    assert!(!by_name("v0.1")["hash"].as_str().unwrap_or("").is_empty());
    assert!(by_name("v0.1")["date"].as_str().unwrap_or("").contains('-'));
}

#[tokio::test]
async fn tags_are_newest_first() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    run_git(dir, &["tag", "-a", "old", "-m", "old tag"]);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    run_git(dir, &["tag", "-a", "new", "-m", "new tag"]);
    let state = state_for(dir);

    let body = tags(&state).await;
    let list = body["tags"].as_array().expect("tags array");
    assert_eq!(list[0]["name"], "new");
    assert_eq!(list[1]["name"], "old");
}

#[tokio::test]
async fn tag_subject_containing_a_tab_survives() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    run_git(dir, &["tag", "-a", "v1", "-m", "before\tafter"]);
    let state = state_for(dir);

    let body = tags(&state).await;
    let list = body["tags"].as_array().expect("tags array");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "v1");
    assert_eq!(list[0]["subject"], "before\tafter");
}

#[tokio::test]
async fn create_and_delete_round_trip() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    let state = state_for(dir);

    let created = create(&state, tag_req("v9", Some("release nine"), None)).await;
    assert_eq!(created["ok"], true);
    let body = tags(&state).await;
    assert_eq!(body["tags"][0]["name"], "v9");
    assert_eq!(body["tags"][0]["subject"], "release nine");

    let deleted = body_json(
        git_tag_delete(
            State(state.clone()),
            auth(),
            Json(GitTagDeleteRequest {
                name: "v9".into(),
                repo: String::new(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(deleted["ok"], true);
    assert!(tags(&state).await["tags"]
        .as_array()
        .expect("array")
        .is_empty());
}

#[tokio::test]
async fn create_on_explicit_target() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    let first = String::from_utf8_lossy(&run_git(dir, &["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_string();
    commit_as(dir, "A", "a@example.com", "a.txt", "two\n", "c2");
    let state = state_for(dir);

    assert_eq!(create(&state, tag_req("at-hash", None, Some(&first))).await["ok"], true);
    assert_eq!(create(&state, tag_req("at-ref", None, Some("main"))).await["ok"], true);

    let resolved = String::from_utf8_lossy(&run_git(dir, &["rev-list", "-n1", "at-hash"]).stdout)
        .trim()
        .to_string();
    assert_eq!(resolved, first);
    let body = tags(&state).await;
    assert_eq!(body["tags"].as_array().expect("array").len(), 2);
}

#[tokio::test]
async fn duplicate_tag_is_refused_not_an_error() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    let state = state_for(dir);

    assert_eq!(create(&state, tag_req("dup", None, None)).await["ok"], true);
    let again = create(&state, tag_req("dup", None, None)).await;
    assert_eq!(again["ok"], false);
    assert!(again["message"].as_str().unwrap_or("").contains("already exists"));
    assert!(again["hint"].is_string());
}

#[tokio::test]
async fn invalid_tag_name_is_rejected() {
    let td = init_repo();
    let state = state_for(td.path());
    let bad = git_tag_create(State(state), auth(), Json(tag_req("--force", None, None))).await;
    assert!(bad.is_err());
}

// ── blame ───────────────────────────────────────────────────────────

// ── unit helpers ────────────────────────────────────────────────────

#[test]
fn malformed_tag_rows_are_skipped() {
    assert!(parse_tag_row("").is_none());
    assert!(parse_tag_row("only-a-name").is_none());
    assert!(parse_tag_row("\thash\tdate\ts\u{1e}s").is_none());
}
