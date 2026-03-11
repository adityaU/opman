//! Git status, diff, log, stage, unstage, commit, discard handlers.
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;

/// GET /api/git/status — structured git status for the active project.
pub async fn git_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get branch name
    let branch_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get porcelain status
    let status_output = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git status: {e}")))?;
    let status_text = String::from_utf8_lossy(&status_output.stdout);

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in status_text.lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();

        // Untracked
        if index_status == '?' {
            untracked.push(GitFileEntry {
                path,
                status: "?".to_string(),
            });
            continue;
        }

        // Staged changes (index column)
        if index_status != ' ' && index_status != '?' {
            staged.push(GitFileEntry {
                path: path.clone(),
                status: index_status.to_string(),
            });
        }

        // Unstaged changes (worktree column)
        if worktree_status != ' ' && worktree_status != '?' {
            unstaged.push(GitFileEntry {
                path,
                status: worktree_status.to_string(),
            });
        }
    }

    Ok(Json(GitStatusResponse {
        branch,
        staged,
        unstaged,
        untracked,
    }))
}

/// GET /api/git/diff?file=...&staged=... — get diff for a file or all files.
pub async fn git_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["diff".to_string()];
    if query.staged {
        args.push("--cached".to_string());
    }
    if let Some(ref file) = query.file {
        args.push("--".to_string());
        args.push(file.clone());
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;

    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Json(GitDiffResponse { diff }))
}

/// GET /api/git/log?limit=50 — recent commits.
pub async fn git_log(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitLogQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);
    let limit = query.limit.unwrap_or(50).min(500); // Cap at 500 commits

    // Use a delimiter that won't appear in normal commit data
    let format = "%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let output = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", limit),
            &format!("--format={}", format),
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<GitLogEntry> = text
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\x1f').collect();
            if parts.len() >= 5 {
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

    Ok(Json(GitLogResponse { commits }))
}

/// POST /api/git/stage — stage files.
pub async fn git_stage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitStageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["add".to_string()];
    if req.files.is_empty() {
        args.push("-A".to_string()); // Stage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
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
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["restore".to_string(), "--staged".to_string()];
    if req.files.is_empty() {
        args.push(".".to_string()); // Unstage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
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

/// POST /api/git/commit — create a commit.
pub async fn git_commit(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCommitRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    if req.message.trim().is_empty() {
        return Err(WebError::BadRequest("Commit message cannot be empty".into()));
    }

    let output = tokio::process::Command::new("git")
        .args(["commit", "-m", &req.message])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git commit: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!("git commit failed: {stderr}")));
    }

    // Get the hash of the commit we just made
    let hash_output = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit hash: {e}")))?;

    let hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();

    Ok(Json(GitCommitResponse {
        hash,
        message: req.message,
    }))
}

/// POST /api/git/discard — discard unstaged changes for files.
pub async fn git_discard(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitDiscardRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    if req.files.is_empty() {
        return Err(WebError::BadRequest(
            "Must specify files to discard".into(),
        ));
    }

    let mut args = vec!["checkout".to_string(), "--".to_string()];
    args.extend(req.files);

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git checkout: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!(
            "git checkout failed: {stderr}"
        )));
    }

    Ok(StatusCode::OK)
}
