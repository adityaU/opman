//! Git show, branches, checkout, range-diff, pull, stash, gitignore handlers.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::{resolve_project_dir, resolve_repo_dir};

/// Helper: resolve the git working directory, honouring `repo` scope.
async fn git_dir(state: &ServerState, repo: &str) -> WebResult<std::path::PathBuf> {
    if repo.is_empty() || repo == "." {
        let dir = resolve_project_dir(state).await?;
        Ok(std::path::PathBuf::from(dir))
    } else {
        resolve_repo_dir(state, repo).await
    }
}

/// Validate a git object hash (SHA-1 hex string).
fn validate_git_hash(hash: &str) -> WebResult<()> {
    if hash.is_empty() || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WebError::BadRequest("Invalid git hash".into()));
    }
    Ok(())
}

/// Validate a git ref/branch name to prevent argument injection.
fn validate_git_ref(name: &str) -> WebResult<()> {
    if name.is_empty()
        || name.starts_with('-')
        || name.contains("..")
        || name.contains('~')
        || name.contains('^')
        || name.contains(':')
    {
        return Err(WebError::BadRequest("Invalid git ref name".into()));
    }
    Ok(())
}

/// Validate a filename to prevent argument injection in git commands.
fn validate_git_filename(name: &str) -> WebResult<()> {
    if name.is_empty() || name.starts_with('-') {
        return Err(WebError::BadRequest("Invalid filename".into()));
    }
    Ok(())
}

/// GET /api/git/show?hash=...&repo=... — show a commit's diff and metadata.
pub async fn git_show(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitShowQuery>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &query.repo).await?;

    validate_git_hash(&query.hash)?;

    // Get commit metadata
    let format = "%H%x1f%an%x1f%aI%x1f%B";
    let meta_output = tokio::process::Command::new("git")
        .args([
            "show",
            "--no-patch",
            &format!("--format={}", format),
            &query.hash,
        ])
        .current_dir(&dir_path)
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
        (
            query.hash.clone(),
            String::new(),
            String::new(),
            String::new(),
        )
    };

    // Get diff
    let diff_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--patch", &query.hash])
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit diff: {e}")))?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Get changed files list
    let files_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--name-status", &query.hash])
        .current_dir(&dir_path)
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

/// GET /api/git/range-diff — get commit log + cumulative diff between base branch and HEAD.
pub async fn git_range_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRangeDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir_path = git_dir(&state, &query.repo).await?;
    let base = query.base.unwrap_or_else(|| "main".to_string());
    validate_git_ref(&base)?;
    let limit = query.limit.unwrap_or(50);

    // Get current branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git rev-parse: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_out.stdout)
        .trim()
        .to_string();

    // Get commits in range base..HEAD
    let log_out = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("{}..HEAD", base),
            &format!("--max-count={}", limit),
            "--format=%H\x1f%h\x1f%an\x1f%aI\x1f%s",
        ])
        .current_dir(&dir_path)
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
        .current_dir(&dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;
    let diff = String::from_utf8_lossy(&diff_out.stdout).to_string();

    // Count files changed
    let stat_out = tokio::process::Command::new("git")
        .args(["diff", &format!("{}...HEAD", base), "--stat"])
        .current_dir(&dir_path)
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

fn combined_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };
    combined.trim().to_string()
}

#[cfg(test)]
#[path = "git_ext_handlers_history_tests.rs"]
pub(crate) mod git_ext_handlers_history_tests;

#[cfg(test)]
#[path = "git_ext_handlers_stash_tests.rs"]
mod git_ext_handlers_stash_tests;
