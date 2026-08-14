//! Turning a request's `repo` field into a directory to run git in.
//!
//! The rule is unchanged from the original handlers — the target must stay
//! inside the active project — with one addition: a linked worktree keeps its
//! `.git` as a *file* rather than a directory, so the repository check tests
//! for existence rather than for a directory.

use std::path::{Path, PathBuf};

use crate::web::error::{WebError, WebResult};
use crate::web::types::ServerState;

/// The directory a git command should run in, plus how it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoScope {
    dir: PathBuf,
}

impl RepoScope {
    pub fn path(&self) -> &Path {
        &self.dir
    }
}

/// Resolve `repo` against the active project.
///
/// An empty or `"."` scope is the project root and is returned without a
/// repository check, matching the behaviour the existing endpoints rely on.
pub async fn resolve(state: &ServerState, repo: &str) -> WebResult<RepoScope> {
    let root = state
        .web_state
        .get_working_dir()
        .await
        .ok_or_else(|| WebError::BadRequest("No active project".into()))?;

    if repo.is_empty() || repo == "." {
        return Ok(RepoScope { dir: root });
    }

    let canonical_root = root
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve project root: {e}")))?;
    let target = root
        .join(repo)
        .canonicalize()
        .map_err(|_| WebError::NotFound("Repository path not found"))?;

    if !target.starts_with(&canonical_root) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }
    if !target.join(".git").exists() {
        return Err(WebError::BadRequest(format!("Not a git repository: {repo}")));
    }

    Ok(RepoScope { dir: target })
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod scope_tests;
