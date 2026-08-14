//! Stash listing and manipulation.
//!
//! One endpoint covers five actions. The action is an enum rather than a
//! string, so the argv for each is built once and a new action cannot be added
//! without deciding whether it takes a ref — which is how the old string form
//! ended up validating `pop`'s ref but not `drop`'s.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::types::*;
use crate::web::git::{exec, refname, scope, Reach, StashRef};

/// POST /api/git/stash — list, save, restore or discard stashes.
pub async fn git_stash(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitStashRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();

    if req.action == GitStashAction::List {
        return Ok(Json(list(dir).await?));
    }

    // Validate before building argv: every ref-taking action shares this, so
    // none of them can skip it.
    let entry = match req.stash_ref.as_deref() {
        Some(raw) => Some(StashRef::parse(raw)?),
        None => None,
    };
    let message = match req.message.as_deref() {
        Some(raw) => Some(refname::message(raw)?),
        None => None,
    };

    let mut args = vec!["stash", req.action.verb()];
    if req.action == GitStashAction::Push {
        // Untracked files are invisible to a plain `stash push`, which makes
        // "stash everything then switch branches" quietly leave them behind.
        args.push("--include-untracked");
        if let Some(message) = message.as_deref() {
            args.extend_from_slice(&["-m", message]);
        }
    } else if let Some(entry) = entry {
        args.push(entry.as_str());
    }

    let action = GitActionResponse::from(exec::run(dir, &args, Reach::Local).await?);
    Ok(Json(GitStashResponse {
        action,
        entries: Vec::new(),
    }))
}

/// Read the stash list, pairing each entry with the ref that addresses it.
async fn list(dir: &std::path::Path) -> WebResult<GitStashResponse> {
    let output = exec::run_lenient(
        dir,
        // `%x09` and not `%09`: pretty-format has no `%09` escape, so the
        // latter prints literally and every column lands in one field.
        &["stash", "list", "--format=%gd%x09%s%x09%cr%x09%h"],
    )
    .await?;

    let entries = output
        .stdout
        .lines()
        .enumerate()
        .filter_map(|(index, line)| parse_entry(index, line))
        .collect();

    Ok(GitStashResponse {
        action: GitActionResponse {
            ok: true,
            failure: None,
            hint: None,
            message: String::new(),
        },
        entries,
    })
}

fn parse_entry(index: usize, line: &str) -> Option<GitStashEntry> {
    let mut fields = line.splitn(4, '\t');
    let reference = fields.next()?.trim();
    if reference.is_empty() {
        return None;
    }
    Some(GitStashEntry {
        index,
        reference: reference.to_string(),
        message: fields.next().unwrap_or_default().trim().to_string(),
        age: fields.next().unwrap_or_default().trim().to_string(),
        hash: fields.next().unwrap_or_default().trim().to_string(),
    })
}

#[cfg(test)]
#[path = "git_stash_tests.rs"]
mod git_stash_tests;
