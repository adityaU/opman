//! Git show, branches, checkout, range-diff, and context-summary handlers.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;

/// GET /api/git/show?hash=... — show a commit's diff and metadata.
pub async fn git_show(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitShowQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get commit metadata
    let format = "%H%x1f%an%x1f%aI%x1f%B";
    let meta_output = tokio::process::Command::new("git")
        .args(["show", "--no-patch", &format!("--format={}", format), &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git show: {e}")))?;

    if !meta_output.status.success() {
        let stderr = String::from_utf8_lossy(&meta_output.stderr);
        return Err(WebError::BadRequest(format!("git show failed: {stderr}")));
    }

    let meta_text = String::from_utf8_lossy(&meta_output.stdout);
    let meta_parts: Vec<&str> = meta_text.trim().splitn(4, '\x1f').collect();
    let (hash, author, date, message) = if meta_parts.len() >= 4 {
        (
            meta_parts[0].to_string(),
            meta_parts[1].to_string(),
            meta_parts[2].to_string(),
            meta_parts[3].trim().to_string(),
        )
    } else {
        (query.hash.clone(), String::new(), String::new(), String::new())
    };

    // Get diff
    let diff_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--patch", &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit diff: {e}")))?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Get changed files list
    let files_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--name-status", &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit files: {e}")))?;

    let files_text = String::from_utf8_lossy(&files_output.stdout);
    let files: Vec<GitShowFile> = files_text
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                Some(GitShowFile {
                    status: parts[0].to_string(),
                    path: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(Json(GitShowResponse {
        hash,
        author,
        date,
        message,
        diff,
        files,
    }))
}

/// GET /api/git/branches — list all local and remote branches.
pub async fn git_branches(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get current branch
    let head_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?;
    let current = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();

    // Get local branches
    let local_output = tokio::process::Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to list local branches: {e}")))?;
    let local: Vec<String> = String::from_utf8_lossy(&local_output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Get remote branches
    let remote_output = tokio::process::Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to list remote branches: {e}")))?;
    let remote: Vec<String> = String::from_utf8_lossy(&remote_output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.contains("HEAD"))
        .collect();

    Ok(Json(GitBranchesResponse {
        current,
        local,
        remote,
    }))
}

/// POST /api/git/checkout — switch to a different branch.
pub async fn git_checkout(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCheckoutRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Validate branch name (basic safety check)
    if req.branch.is_empty()
        || req.branch.contains("..")
        || req.branch.contains("~")
        || req.branch.starts_with('-')
    {
        return Err(WebError::BadRequest("Invalid branch name".to_string()));
    }

    let output = tokio::process::Command::new("git")
        .args(["checkout", &req.branch])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git checkout: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(Json(GitCheckoutResponse {
            branch: req.branch,
            success: true,
            message: if stderr.is_empty() { None } else { Some(stderr) },
        }))
    } else {
        Ok(Json(GitCheckoutResponse {
            branch: req.branch,
            success: false,
            message: Some(if stderr.is_empty() {
                "Checkout failed".to_string()
            } else {
                stderr
            }),
        }))
    }
}

/// GET /api/git/range-diff — get commit log + cumulative diff between base branch and HEAD.
///
/// Useful for "Draft PR Description" — gathers all commits and changes relative to a base branch.
pub async fn git_range_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRangeDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);
    let base = query.base.unwrap_or_else(|| "main".to_string());
    let limit = query.limit.unwrap_or(50);

    // Get current branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git rev-parse: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();

    // Get commits in range base..HEAD
    let log_out = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("{}..HEAD", base),
            &format!("--max-count={}", limit),
            "--format=%H\x1f%h\x1f%an\x1f%aI\x1f%s",
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let commits: Vec<GitLogEntry> = String::from_utf8_lossy(&log_out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '\x1f').collect();
            if parts.len() == 5 {
                Some(GitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    // Get cumulative diff
    let diff_out = tokio::process::Command::new("git")
        .args(["diff", &format!("{}...HEAD", base)])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;
    let diff = String::from_utf8_lossy(&diff_out.stdout).to_string();

    // Count files changed
    let stat_out = tokio::process::Command::new("git")
        .args(["diff", &format!("{}...HEAD", base), "--stat"])
        .current_dir(dir_path)
        .output()
        .await
        .ok();
    let files_changed = stat_out
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| l.contains('|'))
                .count()
        })
        .unwrap_or(0);

    Ok(Json(GitRangeDiffResponse {
        branch,
        base,
        commits,
        diff,
        files_changed,
    }))
}
