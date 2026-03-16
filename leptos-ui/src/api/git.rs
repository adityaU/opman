//! Git API helpers — repos, status, log, show, diff, branches, stage, unstage, commit, discard, checkout, range-diff, context-summary.

use serde::Serialize;

use super::client::{api_fetch, api_post, api_post_void, ApiError};
use crate::types::api::*;

fn repo_query(repo: &str) -> String {
    format!("repo={}", js_sys::encode_uri_component(repo))
}

/// List available git repos.
pub async fn git_repos() -> Result<GitReposResponse, ApiError> {
    api_fetch("/git/repos").await
}

/// Get git status for a repo.
pub async fn git_status(repo: &str) -> Result<GitStatusResponse, ApiError> {
    api_fetch(&format!("/git/status?{}", repo_query(repo))).await
}

/// Get git log for a repo.
pub async fn git_log(repo: &str, limit: Option<usize>) -> Result<GitLogResponse, ApiError> {
    let mut path = format!("/git/log?{}", repo_query(repo));
    if let Some(n) = limit {
        path.push_str(&format!("&limit={}", n));
    }
    api_fetch(&path).await
}

/// Get full commit details.
pub async fn git_show(repo: &str, hash: &str) -> Result<GitShowResponse, ApiError> {
    let encoded_hash = js_sys::encode_uri_component(hash);
    api_fetch(&format!("/git/show?{}&hash={}", repo_query(repo), encoded_hash)).await
}

/// Get diff for a file (or all files).
pub async fn git_diff(repo: &str, file: Option<&str>, staged: bool) -> Result<GitDiffResponse, ApiError> {
    let mut path = format!("/git/diff?{}&staged={}", repo_query(repo), staged);
    if let Some(f) = file {
        path.push_str(&format!("&file={}", js_sys::encode_uri_component(f)));
    }
    api_fetch(&path).await
}

/// Get branches for a repo.
pub async fn git_branches(repo: &str) -> Result<GitBranchesResponse, ApiError> {
    api_fetch(&format!("/git/branches?{}", repo_query(repo))).await
}

/// Get range-diff for a branch relative to a base branch.
pub async fn git_range_diff(repo: &str, base: Option<&str>) -> Result<GitRangeDiffResponse, ApiError> {
    let mut path = format!("/git/range-diff?{}", repo_query(repo));
    if let Some(b) = base {
        path.push_str(&format!("&base={}", js_sys::encode_uri_component(b)));
    }
    api_fetch(&path).await
}

/// Get context summary for a repo.
pub async fn git_context_summary(repo: &str) -> Result<GitContextSummaryResponse, ApiError> {
    api_fetch(&format!("/git/context-summary?{}", repo_query(repo))).await
}

// ── Mutation endpoints ─────────────────────────────────────────────

#[derive(Serialize)]
struct FilePathBody<'a> {
    repo: &'a str,
    path: &'a str,
}

/// Stage a file.
pub async fn git_stage(repo: &str, path: &str) -> Result<(), ApiError> {
    api_post_void("/git/stage", &FilePathBody { repo, path }).await
}

/// Unstage a file.
pub async fn git_unstage(repo: &str, path: &str) -> Result<(), ApiError> {
    api_post_void("/git/unstage", &FilePathBody { repo, path }).await
}

/// Discard changes to a file.
pub async fn git_discard(repo: &str, path: &str) -> Result<(), ApiError> {
    api_post_void("/git/discard", &FilePathBody { repo, path }).await
}

#[derive(Serialize)]
struct CommitBody<'a> {
    repo: &'a str,
    message: &'a str,
}

/// Commit staged changes.
pub async fn git_commit(repo: &str, message: &str) -> Result<GitCommitResponse, ApiError> {
    api_post("/git/commit", &CommitBody { repo, message }).await
}

#[derive(Serialize)]
struct CheckoutBody<'a> {
    repo: &'a str,
    branch: &'a str,
}

/// Checkout a branch.
pub async fn git_checkout(repo: &str, branch: &str) -> Result<GitCheckoutResponse, ApiError> {
    api_post("/git/checkout", &CheckoutBody { repo, branch }).await
}
