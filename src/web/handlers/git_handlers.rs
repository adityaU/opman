//! Git status, diff, log, stage, unstage, commit, discard handlers.
use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::{resolve_project_dir, resolve_repo_dir};
use crate::web::git::{exec, refname, scope, Reach};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

/// Helper: resolve the git working directory, honouring `repo` scope query param.
async fn git_dir(state: &ServerState, repo: &str) -> WebResult<std::path::PathBuf> {
    if repo.is_empty() || repo == "." {
        let dir = resolve_project_dir(state).await?;
        Ok(std::path::PathBuf::from(dir))
    } else {
        resolve_repo_dir(state, repo).await
    }
}

/// GET /api/git/status?repo=... — structured git status for a repo.
pub async fn git_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(scope): axum::extract::Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &scope.repo).await?;

    // Get branch name
    let branch_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get porcelain status
    let status_output = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(&dir_path)
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

/// GET /api/git/diff?file=...&staged=...&repo=... — get diff for a file or all files.
pub async fn git_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &query.repo).await?;

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
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;

    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Json(GitDiffResponse { diff }))
}

/// GET /api/git/log?limit=50&repo=... — recent commits.
pub async fn git_log(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitLogQuery>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &query.repo).await?;
    let limit = query.limit.unwrap_or(50).min(500); // Cap at 500 commits

    // Use a delimiter that won't appear in normal commit data
    let format = "%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let output = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", limit),
            &format!("--format={}", format),
        ])
        .current_dir(&dir_path)
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

#[cfg(test)]
#[path = "git_handlers_tests.rs"]
pub(crate) mod git_handlers_tests;
