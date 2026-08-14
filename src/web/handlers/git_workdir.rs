//! Working-directory mutations: stage, unstage, commit, discard.
//!
//! Split from the read handlers because these are the ones that can refuse in
//! a way the panel offers a recovery for, and they share the argv-validation
//! discipline in [`crate::web::git::refname`].

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::{resolve_project_dir, resolve_repo_dir};
use crate::web::git::{exec, refname, scope, Reach};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

/// Helper: resolve the git working directory, honouring `repo` scope.
async fn git_dir(state: &ServerState, repo: &str) -> WebResult<std::path::PathBuf> {
    if repo.is_empty() || repo == "." {
        let dir = resolve_project_dir(state).await?;
        Ok(std::path::PathBuf::from(dir))
    } else {
        resolve_repo_dir(state, repo).await
    }
}

/// POST /api/git/stage — stage files.
pub async fn git_stage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitStageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &req.repo).await?;

    // Validate filenames to prevent argument injection
    for f in &req.files {
        if f.is_empty() || f.starts_with('-') {
            return Err(WebError::BadRequest("Invalid filename".into()));
        }
    }

    let mut args = vec!["add".to_string()];
    if req.files.is_empty() {
        args.push("-A".to_string()); // Stage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git add: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!("git add failed: {stderr}")));
    }

    Ok(StatusCode::OK)
}

/// POST /api/git/unstage — unstage files.
pub async fn git_unstage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitUnstageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &req.repo).await?;

    // Validate filenames to prevent argument injection
    for f in &req.files {
        if f.is_empty() || f.starts_with('-') {
            return Err(WebError::BadRequest("Invalid filename".into()));
        }
    }

    let mut args = vec!["restore".to_string(), "--staged".to_string()];
    if req.files.is_empty() {
        args.push(".".to_string()); // Unstage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git restore: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!(
            "git restore --staged failed: {stderr}"
        )));
    }

    Ok(StatusCode::OK)
}

/// POST /api/git/commit — create a commit, or replace the previous one.
///
/// A refusal here (nothing staged, a failing pre-commit hook) is an answer the
/// UI renders inline, so it comes back as a `GitActionResponse` rather than a
/// 500 that would strip git's explanation down to a toast.
pub async fn git_commit(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCommitRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();
    let message = refname::message(&req.message)?;

    if req.stage_all {
        let staged = exec::run(dir, &["add", "-u"], Reach::Local).await?;
        if let Err(refusal) = staged {
            return Ok(Json(GitCommitResponse {
                action: GitActionResponse::refused(refusal),
                hash: None,
            }));
        }
    }

    let mut args = vec!["commit", "-m", message.as_ref()];
    if req.amend {
        args.push("--amend");
    }

    let action = GitActionResponse::from(exec::run(dir, &args, Reach::Local).await?);
    let hash = match action.ok {
        true => exec::run_lenient(dir, &["rev-parse", "HEAD"])
            .await
            .map(|out| out.trimmed().to_string())
            .ok()
            .filter(|h| !h.is_empty()),
        false => None,
    };

    Ok(Json(GitCommitResponse { action, hash }))
}

/// POST /api/git/discard — discard unstaged changes for files.
pub async fn git_discard(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitDiscardRequest>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &req.repo).await?;

    if req.files.is_empty() {
        return Err(WebError::BadRequest("Must specify files to discard".into()));
    }

    // Validate filenames to prevent argument injection
    for f in &req.files {
        if f.is_empty() || f.starts_with('-') {
            return Err(WebError::BadRequest("Invalid filename".into()));
        }
    }

    let mut args = vec!["checkout".to_string(), "--".to_string()];
    args.extend(req.files);

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git checkout: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!("git checkout failed: {stderr}")));
    }

    Ok(StatusCode::OK)
}


#[cfg(test)]
#[path = "git_workdir_tests.rs"]
pub(crate) mod git_workdir_tests;

#[cfg(test)]
#[path = "git_commit_tests.rs"]
mod git_commit_tests;
