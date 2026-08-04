//! Generated coverage tests for `handlers/common.rs`.

use super::*;

use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

/// Build a ServerState whose active project points at `p`.
fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

// ── constant_time_eq ───────────────────────────────────────────────

#[test]
fn constant_time_eq_matches() {
    assert!(constant_time_eq(b"hello", b"hello"));
    assert!(constant_time_eq(b"", b""));
}

#[test]
fn constant_time_eq_differs() {
    assert!(!constant_time_eq(b"hello", b"world"));
    // different lengths
    assert!(!constant_time_eq(b"abc", b"abcd"));
}

// ── resolve_project_dir ────────────────────────────────────────────

#[tokio::test]
async fn resolve_project_dir_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let d = resolve_project_dir(&state).await.unwrap();
    assert_eq!(d, tmp.path().to_string_lossy());
}

#[tokio::test]
async fn resolve_project_dir_no_project() {
    let state = test_server_state();
    let res = resolve_project_dir(&state).await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

// ── resolve_readable_path ──────────────────────────────────────────

#[test]
fn resolve_readable_path_relative_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hi").unwrap();
    let out = resolve_readable_path(tmp.path(), "a.txt").unwrap();
    assert!(out.ends_with("a.txt"));
}

#[test]
fn resolve_readable_path_absolute_ok() {
    let tmp = tempfile::TempDir::new().unwrap();
    let f = tmp.path().join("b.txt");
    std::fs::write(&f, "hi").unwrap();
    let out = resolve_readable_path(tmp.path(), &f.to_string_lossy()).unwrap();
    assert!(out.ends_with("b.txt"));
}

#[test]
fn resolve_readable_path_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let res = resolve_readable_path(tmp.path(), "nope.txt");
    assert!(matches!(res, Err(WebError::NotFound(_))));
}

#[test]
fn resolve_readable_path_outside_project() {
    // A real file that exists but is outside the project (and outside ~/.claude).
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    let outside = root.path().join("secret.txt");
    std::fs::write(&outside, "x").unwrap();
    let res = resolve_readable_path(&proj, &outside.to_string_lossy());
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

// ── resolve_repo_dir ───────────────────────────────────────────────

#[tokio::test]
async fn resolve_repo_dir_root_variants() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let a = resolve_repo_dir(&state, "").await.unwrap();
    let b = resolve_repo_dir(&state, ".").await.unwrap();
    assert_eq!(a, tmp.path().to_path_buf());
    assert_eq!(b, a);
}

#[tokio::test]
async fn resolve_repo_dir_valid_git_repo() {
    let tmp = tempfile::TempDir::new().unwrap();
    let repo = tmp.path().join("sub");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(repo.join(".git")).unwrap();
    let state = state_dir(tmp.path());
    let out = resolve_repo_dir(&state, "sub").await.unwrap();
    assert!(out.ends_with("sub"));
}

#[tokio::test]
async fn resolve_repo_dir_not_a_git_repo() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("plain")).unwrap();
    let state = state_dir(tmp.path());
    let res = resolve_repo_dir(&state, "plain").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[tokio::test]
async fn resolve_repo_dir_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let res = resolve_repo_dir(&state, "ghost").await;
    assert!(matches!(res, Err(WebError::NotFound(_))));
}

#[tokio::test]
async fn resolve_repo_dir_traversal() {
    let root = tempfile::TempDir::new().unwrap();
    let proj = root.path().join("proj");
    std::fs::create_dir(&proj).unwrap();
    // sibling git repo outside the project
    let sib = root.path().join("outside");
    std::fs::create_dir(&sib).unwrap();
    std::fs::create_dir(sib.join(".git")).unwrap();
    let state = state_dir(&proj);
    let res = resolve_repo_dir(&state, "../outside").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[tokio::test]
async fn resolve_repo_dir_no_project() {
    let state = test_server_state();
    let res = resolve_repo_dir(&state, "x").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

// ── resolve_editor_nvim_socket / resolve_editor_buffer ─────────────

#[tokio::test]
async fn resolve_editor_nvim_socket_absent() {
    let state = test_server_state();
    let res = resolve_editor_nvim_socket(&state, "sess").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[tokio::test]
async fn resolve_editor_nvim_socket_present() {
    let state = test_server_state();
    {
        let mut reg = state.nvim_registry.write().await;
        reg.insert(
            (0, "sess".to_string()),
            std::path::PathBuf::from("/tmp/nonexistent.sock"),
        );
    }
    let out = resolve_editor_nvim_socket(&state, "sess").await.unwrap();
    assert_eq!(out, std::path::PathBuf::from("/tmp/nonexistent.sock"));
}

#[tokio::test]
async fn resolve_editor_buffer_no_socket() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let res = resolve_editor_buffer(&state, "sess", "a.rs").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[tokio::test]
async fn resolve_editor_buffer_no_working_dir() {
    // socket present but no active project dir → BadRequest.
    let state = test_server_state();
    {
        let mut reg = state.nvim_registry.write().await;
        reg.insert(
            (0, "sess".to_string()),
            std::path::PathBuf::from("/tmp/nonexistent.sock"),
        );
    }
    let res = resolve_editor_buffer(&state, "sess", "a.rs").await;
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[tokio::test]
async fn resolve_editor_buffer_bogus_socket_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    {
        let mut reg = state.nvim_registry.write().await;
        reg.insert((0, "sess".to_string()), tmp.path().join("no-such.sock"));
    }
    // Connecting to a non-existent unix socket fails → Internal error.
    let res = resolve_editor_buffer(&state, "sess", "a.rs").await;
    assert!(matches!(res, Err(WebError::Internal(_))));
}

#[tokio::test]
async fn resolve_editor_buffer_absolute_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    {
        let mut reg = state.nvim_registry.write().await;
        reg.insert((0, "sess".to_string()), tmp.path().join("no-such.sock"));
    }
    // absolute path branch
    let abs = tmp.path().join("x.rs");
    let res = resolve_editor_buffer(&state, "sess", &abs.to_string_lossy()).await;
    assert!(res.is_err());
}
