//! Types for the branch, sync and commit operations added with the full git panel.
//!
//! Mutations answer with [`GitActionResponse`]. A git command that ran and
//! refused is a legitimate answer rather than a transport error, so it comes
//! back as HTTP 200 carrying a machine-readable [`GitFailure`] the UI can
//! offer a specific recovery for. Only bad input and spawn failures are 4xx/5xx.

use serde::{Deserialize, Serialize};

use crate::web::git::{GitFailure, GitOutput, GitRefusal, GitResult};

/// The result of one mutating git operation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitActionResponse {
    pub ok: bool,
    /// Present only when `ok` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<GitFailure>,
    /// What to do next, paired with `failure`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<&'static str>,
    /// Git's own words, shown verbatim so nothing is lost in translation.
    pub message: String,
}

impl GitActionResponse {
    pub fn succeeded(output: &GitOutput) -> Self {
        Self {
            ok: true,
            failure: None,
            hint: None,
            message: output.summary().to_string(),
        }
    }

    pub fn refused(refusal: GitRefusal) -> Self {
        Self {
            ok: false,
            failure: Some(refusal.failure),
            hint: Some(refusal.failure.hint()),
            message: refusal.detail,
        }
    }

    /// Refused for a reason this codebase determined rather than git.
    pub fn blocked(failure: GitFailure, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            failure: Some(failure),
            hint: Some(failure.hint()),
            message: message.into(),
        }
    }
}

impl From<GitResult> for GitActionResponse {
    fn from(result: GitResult) -> Self {
        match result {
            Ok(output) => Self::succeeded(&output),
            Err(refusal) => Self::refused(refusal),
        }
    }
}

/// One branch, local or remote-tracking, with everything the list row shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchInfo {
    pub name: String,
    /// True for the branch HEAD currently points at.
    pub current: bool,
    /// True for remote-tracking refs such as `origin/main`.
    pub remote: bool,
    /// Upstream this branch tracks, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits this branch has that its upstream does not.
    pub ahead: u32,
    /// Commits the upstream has that this branch does not.
    pub behind: u32,
    /// Subject of the branch tip, for orientation without opening the log.
    pub subject: String,
    /// ISO-8601 commit date of the tip.
    pub date: String,
    /// Path of the linked worktree that has this branch checked out, if any.
    /// A branch checked out elsewhere cannot be checked out here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
}

/// Response for `GET /api/git/branches`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesResponse {
    /// Empty in a repository with no commits, or when HEAD is detached.
    pub current: String,
    /// True when HEAD points at a commit rather than a branch.
    pub detached: bool,
    pub local: Vec<GitBranchInfo>,
    pub remote: Vec<GitBranchInfo>,
    /// Configured remote names, in git's order.
    pub remotes: Vec<String>,
}

/// Request body for `POST /api/git/checkout`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckoutRequest {
    /// Branch to switch to. A remote-tracking name such as `origin/feat`
    /// creates the matching local branch rather than detaching HEAD.
    pub branch: String,
    #[serde(default)]
    pub repo: String,
    /// Carry uncommitted changes across instead of refusing on a dirty tree.
    #[serde(default)]
    pub carry_changes: bool,
}

/// Request body for `POST /api/git/branch/create`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchCreateRequest {
    pub name: String,
    /// Commit-ish to branch from. Defaults to HEAD.
    #[serde(default)]
    pub start_point: Option<String>,
    /// Switch to the new branch after creating it.
    #[serde(default)]
    pub checkout: bool,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/branch/delete`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchDeleteRequest {
    pub name: String,
    /// Delete even when the branch is not merged. Destructive.
    #[serde(default)]
    pub force: bool,
    /// Delete the branch on its remote instead of locally.
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/branch/rename`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchRenameRequest {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub repo: String,
}

/// Response for `GET /api/git/sync-status`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSyncStatusResponse {
    pub branch: String,
    pub detached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub remotes: Vec<GitRemoteInfo>,
    /// True when the repository has no commits yet.
    pub unborn: bool,
}

/// A configured remote.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoteInfo {
    pub name: String,
    pub fetch_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_url: Option<String>,
}

/// Request body for `POST /api/git/fetch`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFetchRequest {
    /// Remote to fetch. Defaults to every configured remote.
    #[serde(default)]
    pub remote: Option<String>,
    /// Delete remote-tracking refs whose upstream branch is gone.
    #[serde(default)]
    pub prune: bool,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/push`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushRequest {
    #[serde(default)]
    pub remote: Option<String>,
    /// Branch to push. Defaults to the current branch.
    #[serde(default)]
    pub branch: Option<String>,
    /// Publish a branch that has no upstream yet.
    #[serde(default)]
    pub set_upstream: bool,
    /// Overwrite the remote history, refusing if it moved since the last
    /// fetch. Always `--force-with-lease`, never a bare `--force`.
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub repo: String,
}

/// Request body for `POST /api/git/commit`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitRequest {
    pub message: String,
    /// Replace the previous commit instead of adding one.
    #[serde(default)]
    pub amend: bool,
    /// Stage every tracked modification first.
    #[serde(default)]
    pub stage_all: bool,
    #[serde(default)]
    pub repo: String,
}

/// Response for `POST /api/git/commit`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitResponse {
    #[serde(flatten)]
    pub action: GitActionResponse,
    /// Hash of the new commit, when one was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[cfg(test)]
#[path = "git_ops_tests.rs"]
mod git_ops_tests;
