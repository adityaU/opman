//! `.gitignore` reading and editing.
//!
//! The only git endpoint that touches no git process at all — a `.gitignore`
//! is an ordinary file, and reading it directly avoids a spawn per keystroke
//! in the editor that calls this.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use crate::web::git::scope;

/// POST /api/git/gitignore — list or add patterns to .gitignore.
pub async fn git_gitignore(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitIgnoreRequest>,
) -> WebResult<impl IntoResponse> {
    let scoped = scope::resolve(&state, &req.repo).await?;
    let dir_path = scoped.path();
    let gitignore_path = dir_path.join(".gitignore");

    match req.action.as_str() {
        "list" | "" => {
            let content = if gitignore_path.exists() {
                tokio::fs::read_to_string(&gitignore_path)
                    .await
                    .unwrap_or_default()
            } else {
                String::new()
            };
            Ok(Json(GitIgnoreResponse {
                success: true,
                content,
            }))
        }
        "add" => {
            if req.patterns.is_empty() {
                return Err(WebError::BadRequest(
                    "Must specify at least one pattern to add".into(),
                ));
            }

            // Read existing content
            let mut content = if gitignore_path.exists() {
                tokio::fs::read_to_string(&gitignore_path)
                    .await
                    .unwrap_or_default()
            } else {
                String::new()
            };

            // Ensure trailing newline before appending
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }

            // Append new patterns (skip duplicates)
            let existing_lines: std::collections::HashSet<String> =
                content.lines().map(|s| s.to_string()).collect();
            for pattern in &req.patterns {
                let trimmed = pattern.trim();
                if !trimmed.is_empty() && !existing_lines.contains(trimmed) {
                    content.push_str(trimmed);
                    content.push('\n');
                }
            }

            tokio::fs::write(&gitignore_path, &content)
                .await
                .map_err(|e| WebError::Internal(format!("Failed to write .gitignore: {e}")))?;

            Ok(Json(GitIgnoreResponse {
                success: true,
                content,
            }))
        }
        other => Err(WebError::BadRequest(format!(
            "Unknown gitignore action: {other}. Supported: list, add"
        ))),
    }
}

/// Helper: combine stdout + stderr from a process output.

#[cfg(test)]
#[path = "git_ignore_tests.rs"]
mod git_ignore_tests;
