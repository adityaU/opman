//! Worktree listing and lifecycle.
//!
//! `git worktree list --porcelain` is a record stream: blank-line separated
//! groups of `key [value]` lines. It is parsed into [`Record`] first and only
//! then joined with the two paths that give an entry its meaning — the project
//! root (for `relative`) and the scoped directory (for `current`) — so the
//! parser stays a pure function the tests can drive with fixture text.

use std::path::{Component, Path, PathBuf};

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::git::{exec, scope, GitFailure, Reach, RefName, RepoPath};
use super::super::types::*;

/// One record of `git worktree list --porcelain`, before it is given context.
#[derive(Debug, Default, PartialEq, Eq)]
struct Record {
    path: Option<String>,
    head: Option<String>,
    /// Full ref, `refs/heads/...` still attached.
    branch: Option<String>,
    bare: bool,
    detached: bool,
    locked: bool,
    prunable: Option<String>,
}

impl Record {
    /// Apply one `key [value]` line.
    fn absorb(&mut self, line: &str) {
        let (key, value) = match line.split_once(' ') {
            Some((key, rest)) => (key, rest.trim()),
            None => (line, ""),
        };
        let owned = || (!value.is_empty()).then(|| value.to_string());
        match key {
            "worktree" => self.path = owned(),
            "HEAD" => self.head = owned(),
            "branch" => self.branch = owned(),
            "bare" => self.bare = true,
            "detached" => self.detached = true,
            "locked" => self.locked = true,
            "prunable" => self.prunable = Some(value.to_string()),
            _ => {}
        }
    }
}

/// Split the porcelain stream into records. Blank lines are the separator.
fn parse_porcelain(stdout: &str) -> Vec<Record> {
    let mut records = Vec::new();
    let mut current = Record::default();
    let mut open = false;

    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if open {
                records.push(std::mem::take(&mut current));
                open = false;
            }
            continue;
        }
        current.absorb(line);
        open = true;
    }
    if open {
        records.push(current);
    }
    records
}

/// Canonical form when the path exists, the path itself otherwise.
fn canon(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Turn a parsed record into a response entry. `None` for a record with no path.
fn to_entry(record: &Record, main: bool, root: &Path, scoped: &Path) -> Option<GitWorktreeEntry> {
    let path = record.path.as_ref()?;
    let real = canon(Path::new(path));
    let relative = real.strip_prefix(root).ok().map(|rel| {
        let text = rel.to_string_lossy().into_owned();
        if text.is_empty() {
            ".".to_string()
        } else {
            text
        }
    });

    Some(GitWorktreeEntry {
        path: path.clone(),
        relative,
        branch: record
            .branch
            .as_ref()
            .map(|full| full.strip_prefix("refs/heads/").unwrap_or(full).to_string()),
        head: record.head.clone().unwrap_or_default(),
        main,
        current: real == scoped,
        locked: record.locked,
        prunable: record.prunable.clone(),
    })
}

/// Resolve a caller-supplied worktree path against the project root, refusing
/// anything that leaves it.
///
/// The destination of an `add` does not exist yet, so containment is decided
/// lexically: `..` is rejected outright rather than resolved away.
fn resolve_inside(root: &Path, raw: &str) -> WebResult<PathBuf> {
    let candidate = Path::new(raw);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };

    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                return Err(WebError::BadRequest(
                    "Worktree path escapes the project".into(),
                ));
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    if !out.starts_with(root) {
        return Err(WebError::BadRequest(
            "Worktree path escapes the project".into(),
        ));
    }
    Ok(out)
}

/// A path as an argv string, which requires it to be UTF-8.
fn as_arg(path: &Path) -> WebResult<&str> {
    path.to_str()
        .ok_or_else(|| WebError::BadRequest("Worktree path is not valid UTF-8".into()))
}

/// Path of the main worktree — the first record git reports.
async fn main_worktree(dir: &Path) -> WebResult<Option<PathBuf>> {
    let output = exec::run_lenient(dir, &["worktree", "list", "--porcelain"]).await?;
    Ok(parse_porcelain(&output.stdout)
        .first()
        .and_then(|record| record.path.as_deref())
        .map(|path| canon(Path::new(path))))
}

/// GET /api/git/worktrees — every worktree of the scoped repository.
pub async fn git_worktrees(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let scoped = scope::resolve(&state, &query.repo).await?;
    let root = scope::resolve(&state, "").await?;
    let root_path = canon(root.path());
    let scoped_path = canon(scoped.path());

    let output = exec::run_strict(
        scoped.path(),
        &["worktree", "list", "--porcelain"],
        Reach::Local,
    )
    .await?;

    let worktrees = parse_porcelain(&output.stdout)
        .iter()
        .enumerate()
        .filter_map(|(index, record)| to_entry(record, index == 0, &root_path, &scoped_path))
        .collect();

    Ok(Json(GitWorktreesResponse { worktrees }))
}

/// POST /api/git/worktree/add — create a worktree inside the project.
pub async fn git_worktree_add(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitWorktreeAddRequest>,
) -> WebResult<impl IntoResponse> {
    let path = RepoPath::parse(&req.path)?;
    let branch = RefName::parse(&req.branch)?;
    let start_point = req
        .start_point
        .as_deref()
        .filter(|raw| !raw.is_empty())
        .map(RefName::parse)
        .transpose()?;

    let root = scope::resolve(&state, "").await?;
    let destination = resolve_inside(&canon(root.path()), path.as_str())?;
    let destination = as_arg(&destination)?;

    let scoped = scope::resolve(&state, &req.repo).await?;

    let mut args = vec!["worktree", "add"];
    if req.create {
        args.extend(["-b", branch.as_str(), destination]);
        if let Some(start) = start_point {
            args.push(start.as_str());
        }
    } else {
        args.extend([destination, branch.as_str()]);
    }

    let result = exec::run(scoped.path(), &args, Reach::Local).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// POST /api/git/worktree/remove — detach a linked worktree from the repository.
pub async fn git_worktree_remove(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitWorktreeRemoveRequest>,
) -> WebResult<impl IntoResponse> {
    let path = RepoPath::parse(&req.path)?;
    let root = scope::resolve(&state, "").await?;
    let target = resolve_inside(&canon(root.path()), path.as_str())?;
    let scoped = scope::resolve(&state, &req.repo).await?;

    if main_worktree(scoped.path()).await? == Some(canon(&target)) {
        return Ok(Json(GitActionResponse::blocked(
            GitFailure::Failed,
            "This is the main worktree; it cannot be removed. Delete the repository directory \
             instead, or remove one of its linked worktrees.",
        )));
    }

    let target = as_arg(&target)?;
    let mut args = vec!["worktree", "remove"];
    if req.force {
        args.push("--force");
    }
    args.push(target);

    let result = exec::run(scoped.path(), &args, Reach::Local).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// POST /api/git/worktree/prune — drop administrative files for worktrees whose
/// directory is gone.
pub async fn git_worktree_prune(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let scoped = scope::resolve(&state, &req.repo).await?;
    let result = exec::run(scoped.path(), &["worktree", "prune", "-v"], Reach::Local).await?;
    Ok(Json(GitActionResponse::from(result)))
}

#[cfg(test)]
#[path = "git_worktree_tests.rs"]
pub(crate) mod git_worktree_tests;

#[cfg(test)]
#[path = "git_worktree_parse_tests.rs"]
mod git_worktree_parse_tests;

#[cfg(test)]
#[path = "git_worktree_add_tests.rs"]
mod git_worktree_add_tests;
