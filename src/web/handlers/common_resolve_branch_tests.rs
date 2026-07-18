//! Extra branch coverage for `common.rs::resolve_repo_dir` — the
//! `base.canonicalize()` failure arm (Internal error), which the existing
//! tests never reach because they always point the project at a real dir.

use super::*;

use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

/// Build a ServerState whose active project points at `p` (existence not required —
/// `get_working_dir` returns the stored path verbatim).
fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

#[tokio::test]
async fn resolve_repo_dir_base_canonicalize_error_is_internal() {
    // Project dir does not exist on disk. With a non-empty repo the code skips
    // the early "." return, joins `repo`, then canonicalizes the *base* — which
    // fails because the base does not exist → Internal.
    let tmp = tempfile::TempDir::new().unwrap();
    let missing = tmp.path().join("does-not-exist");
    let state = state_dir(&missing);
    let res = resolve_repo_dir(&state, "sub").await;
    assert!(matches!(res, Err(WebError::Internal(_))));
}

#[tokio::test]
async fn resolve_repo_dir_missing_base_but_dot_returns_ok() {
    // With repo "." the early return fires *before* any canonicalize, so even a
    // non-existent project dir yields Ok(base) — the pre-canonicalize branch.
    let tmp = tempfile::TempDir::new().unwrap();
    let missing = tmp.path().join("nope");
    let state = state_dir(&missing);
    let out = resolve_repo_dir(&state, ".").await.unwrap();
    assert_eq!(out, missing);
}
