//! Git context summary handler for AI session injection, and multi-repo discovery.

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

/// GET /api/git/repos — discover all git repositories under the workspace root.
///
/// Walks the project directory looking for `.git` dirs (up to 4 levels deep).
/// Returns basic status info for each discovered repo so the frontend can
/// show a repo-switcher.
pub async fn git_repos(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);

    let mut repos = Vec::new();

    // Check if the project root itself is a git repo
    if base.join(".git").exists() {
        if let Some(entry) = quick_repo_info(base, ".").await {
            repos.push(entry);
        }
    }

    // Walk up to 4 levels deep looking for nested .git directories
    discover_repos(base, base, 0, 4, &mut repos).await;

    // Sort: root first, then alphabetically by path
    repos.sort_by(|a, b| {
        if a.path == "." {
            std::cmp::Ordering::Less
        } else if b.path == "." {
            std::cmp::Ordering::Greater
        } else {
            a.path.to_lowercase().cmp(&b.path.to_lowercase())
        }
    });

    Ok(Json(GitReposResponse { repos }))
}

/// Recursively discover git repos under `current_dir`, up to `max_depth`.
async fn discover_repos(
    base: &std::path::Path,
    current: &std::path::Path,
    depth: usize,
    max_depth: usize,
    repos: &mut Vec<GitRepoEntry>,
) {
    if depth >= max_depth {
        return;
    }

    let mut dir_reader = match tokio::fs::read_dir(current).await {
        Ok(r) => r,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = dir_reader.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden dirs, node_modules, target, vendor, etc.
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "vendor"
            || name == "dist"
            || name == "build"
            || name == "__pycache__"
        {
            continue;
        }

        let path = entry.path();
        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !metadata.is_dir() {
            continue;
        }

        // Check if this subdir is a git repo
        if path.join(".git").exists() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            // Skip if we already found this repo (root case)
            if repos.iter().any(|r| r.path == rel) {
                continue;
            }
            if let Some(entry) = quick_repo_info(&path, &rel).await {
                repos.push(entry);
            }
            // Don't descend into a git repo looking for more
            continue;
        }

        // Recurse into subdirectory
        Box::pin(discover_repos(base, &path, depth + 1, max_depth, repos)).await;
    }
}

/// Get quick branch and change-count info for a repo.
async fn quick_repo_info(repo_path: &std::path::Path, rel_path: &str) -> Option<GitRepoEntry> {
    // Branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await
        .ok()?;
    let branch = String::from_utf8_lossy(&branch_out.stdout)
        .trim()
        .to_string();

    // Quick status
    let status_out = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(repo_path)
        .output()
        .await
        .ok()?;
    let status_text = String::from_utf8_lossy(&status_out.stdout);

    let mut staged_count = 0usize;
    let mut unstaged_count = 0usize;
    let mut untracked_count = 0usize;
    for line in status_text.lines() {
        if line.len() < 2 {
            continue;
        }
        let idx = line.as_bytes()[0];
        let wt = line.as_bytes()[1];
        if idx == b'?' {
            untracked_count += 1;
        } else {
            if idx != b' ' { staged_count += 1; }
            if wt != b' ' { unstaged_count += 1; }
        }
    }

    let name = if rel_path == "." {
        repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string())
    } else {
        rel_path
            .rsplit('/')
            .next()
            .unwrap_or(rel_path)
            .to_string()
    };

    Some(GitRepoEntry {
        path: rel_path.to_string(),
        name,
        branch,
        staged_count,
        unstaged_count,
        untracked_count,
    })
}
