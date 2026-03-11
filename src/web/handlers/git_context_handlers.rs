//! Git context summary handler for AI session injection.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;

/// GET /api/git/context-summary — structured git context for AI injection.
///
/// Returns current branch, recent commits, change counts, and a human-readable
/// summary suitable for prepending to an AI session.
pub async fn git_context_summary(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get current branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git rev-parse: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();

    // Recent commits (last 5)
    let log_out = tokio::process::Command::new("git")
        .args([
            "log",
            "--max-count=5",
            "--format=%H\x1f%h\x1f%an\x1f%aI\x1f%s",
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let recent_commits: Vec<GitLogEntry> = String::from_utf8_lossy(&log_out.stdout)
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

    // Get status counts
    let status_out = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git status: {e}")))?;

    let status_text = String::from_utf8_lossy(&status_out.stdout);
    let mut staged_count = 0usize;
    let mut unstaged_count = 0usize;
    let mut untracked_count = 0usize;

    for line in status_text.lines() {
        if line.len() < 2 {
            continue;
        }
        let index = line.as_bytes()[0];
        let worktree = line.as_bytes()[1];

        if index == b'?' {
            untracked_count += 1;
        } else {
            if index != b' ' && index != b'?' {
                staged_count += 1;
            }
            if worktree != b' ' && worktree != b'?' {
                unstaged_count += 1;
            }
        }
    }

    // Build human-readable summary
    let mut summary_parts = vec![format!("Branch: {}", branch)];
    if !recent_commits.is_empty() {
        summary_parts.push(format!(
            "Last commit: {} ({})",
            recent_commits[0].message, recent_commits[0].short_hash
        ));
    }
    if staged_count > 0 {
        summary_parts.push(format!("{} file(s) staged", staged_count));
    }
    if unstaged_count > 0 {
        summary_parts.push(format!("{} file(s) modified (unstaged)", unstaged_count));
    }
    if untracked_count > 0 {
        summary_parts.push(format!("{} untracked file(s)", untracked_count));
    }
    if staged_count == 0 && unstaged_count == 0 && untracked_count == 0 {
        summary_parts.push("Working tree clean".to_string());
    }
    let summary = summary_parts.join(". ");

    Ok(Json(GitContextSummaryResponse {
        branch,
        recent_commits,
        staged_count,
        unstaged_count,
        untracked_count,
        summary,
    }))
}
