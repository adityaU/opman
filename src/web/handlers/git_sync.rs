//! Where HEAD stands relative to its upstream, and the three commands that
//! move it: fetch, push, pull.
//!
//! Every remote-touching command runs with [`Reach::Network`] and answers with
//! a [`GitActionResponse`]: git refusing (no credentials, a rejected push) is a
//! real answer the UI can act on, not a transport error.

use std::path::Path;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::error::WebResult;
use crate::web::git::exec::{run, run_lenient};
use crate::web::git::{scope, GitFailure, GitOutput, Reach, RefName};
use crate::web::types::{
    GitActionResponse, GitFetchRequest, GitPullRequest, GitPushRequest, GitRemoteInfo,
    GitRepoScope, GitSyncStatusResponse, ServerState,
};

/// What HEAD currently points at.
enum Head {
    /// The repository has no commits yet.
    Unborn,
    /// Detached, carrying the short hash for display.
    Detached(String),
    Branch(String),
}

/// Read HEAD's shape with the two cheapest plumbing calls that distinguish it.
async fn head_state(dir: &Path) -> WebResult<Head> {
    if run(dir, &["rev-parse", "--verify", "HEAD"], Reach::Local)
        .await?
        .is_err()
    {
        return Ok(Head::Unborn);
    }
    match run(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"], Reach::Local).await? {
        Ok(output) => Ok(Head::Branch(output.trimmed().to_string())),
        Err(_) => {
            let short = run_lenient(dir, &["rev-parse", "--short", "HEAD"]).await?;
            Ok(Head::Detached(short.trimmed().to_string()))
        }
    }
}

/// The upstream ref of the current HEAD, when it tracks one.
async fn upstream_of(dir: &Path) -> WebResult<Option<String>> {
    let result = run(
        dir,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
        Reach::Local,
    )
    .await?;
    Ok(match result {
        Ok(output) => {
            let name = output.trimmed();
            (!name.is_empty()).then(|| name.to_string())
        }
        Err(_) => None,
    })
}

/// `(behind, ahead)` against `upstream`. A missing or unparseable count is 0 —
/// the divergence display is informational and must never fail the request.
async fn divergence(dir: &Path, upstream: &str) -> WebResult<(u32, u32)> {
    let spec = format!("{upstream}...HEAD");
    let output = run_lenient(dir, &["rev-list", "--left-right", "--count", &spec]).await?;
    let mut parts = output.trimmed().split_whitespace();
    let mut next = || parts.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let behind = next();
    let ahead = next();
    Ok((behind, ahead))
}

/// Parse `git remote -v`, which prints a `(fetch)` and a `(push)` line per
/// remote. The push URL is reported only when it actually differs.
fn parse_remotes(output: &GitOutput) -> Vec<GitRemoteInfo> {
    let mut remotes: Vec<GitRemoteInfo> = Vec::new();
    for line in output.lines() {
        let Some((name, rest)) = line.split_once('\t') else {
            continue;
        };
        let Some((url, kind)) = rest.rsplit_once(' ') else {
            continue;
        };
        match kind {
            "(fetch)" if !remotes.iter().any(|r| r.name == name) => {
                remotes.push(GitRemoteInfo {
                    name: name.to_string(),
                    fetch_url: url.to_string(),
                    push_url: None,
                });
            }
            "(push)" => {
                if let Some(entry) = remotes.iter_mut().find(|r| r.name == name) {
                    if entry.fetch_url != url {
                        entry.push_url = Some(url.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    remotes
}

/// GET /api/git/sync-status?repo=... — HEAD, its upstream, and the remotes.
pub async fn git_sync_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &query.repo).await?;
    let dir = repo.path();
    let remotes = parse_remotes(&run_lenient(dir, &["remote", "-v"]).await?);

    let (branch, detached) = match head_state(dir).await? {
        Head::Unborn => {
            return Ok(Json(GitSyncStatusResponse {
                branch: String::new(),
                detached: false,
                upstream: None,
                ahead: 0,
                behind: 0,
                remotes,
                unborn: true,
            }));
        }
        Head::Detached(short) => (short, true),
        Head::Branch(name) => (name, false),
    };

    let upstream = upstream_of(dir).await?;
    let (behind, ahead) = match upstream.as_deref() {
        Some(name) => divergence(dir, name).await?,
        None => (0, 0),
    };

    Ok(Json(GitSyncStatusResponse {
        branch,
        detached,
        upstream,
        ahead,
        behind,
        remotes,
        unborn: false,
    }))
}

/// POST /api/git/fetch — fetch one remote, or every remote when none is named.
pub async fn git_fetch(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitFetchRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let remote = req.remote.as_deref().map(RefName::parse).transpose()?;

    let mut args = vec!["fetch"];
    if req.prune {
        args.push("--prune");
    }
    args.push(remote.map_or("--all", RefName::as_str));

    let result = run(repo.path(), &args, Reach::Network).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// POST /api/git/push — publish the current branch, or a named one.
///
/// Force is always `--force-with-lease`: overwriting a remote that moved since
/// the last fetch would discard someone else's work silently.
pub async fn git_push(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitPushRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();

    if let Some(name) = req.remote.as_deref() {
        RefName::parse(name)?;
    }
    if let Some(name) = req.branch.as_deref() {
        RefName::parse(name)?;
    }

    let head = head_state(dir).await?;
    let branch = match (req.branch, &head) {
        (Some(name), _) => name,
        (None, Head::Branch(name)) => name.clone(),
        (None, _) => {
            return Ok(Json(GitActionResponse::blocked(
                GitFailure::NotFound,
                "HEAD is not on a branch, so there is nothing to push. Check out a branch first.",
            )));
        }
    };

    let remote = match req.remote {
        Some(name) => name,
        None => match default_remote(dir, &head).await? {
            Some(name) => name,
            None => {
                return Ok(Json(GitActionResponse::blocked(
                    GitFailure::NotFound,
                    "No remote configured",
                )));
            }
        },
    };

    let mut args = vec!["push"];
    if req.force {
        args.push("--force-with-lease");
    }
    if req.set_upstream {
        args.push("--set-upstream");
    }
    args.push(&remote);
    args.push(&branch);

    let result = run(dir, &args, Reach::Network).await?;
    Ok(Json(GitActionResponse::from(result)))
}

/// The remote a push should go to when the caller named none: the current
/// branch's configured upstream remote, else `origin` if it exists.
async fn default_remote(dir: &Path, head: &Head) -> WebResult<Option<String>> {
    if let Head::Branch(branch) = head {
        let key = format!("branch.{branch}.remote");
        let configured = run_lenient(dir, &["config", "--get", &key]).await?;
        let name = configured.trimmed();
        if !name.is_empty() {
            return Ok(Some(name.to_string()));
        }
    }
    let listed = run_lenient(dir, &["remote"]).await?;
    let has_origin = listed.lines().any(|r| r == "origin");
    Ok(has_origin.then(|| "origin".to_string()))
}

/// POST /api/git/pull — integrate the upstream, fast-forward only.
///
/// `--ff-only` keeps a pull from silently manufacturing a merge commit; when it
/// refuses, the UI offers an explicit merge or rebase instead.
pub async fn git_pull(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitPullRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;

    let mut args = vec!["pull", "--ff-only"];
    if !req.remote.is_empty() {
        args.push(RefName::parse(&req.remote)?.as_str());
        if !req.branch.is_empty() {
            args.push(RefName::parse(&req.branch)?.as_str());
        }
    }

    let result = run(repo.path(), &args, Reach::Network).await?;
    Ok(Json(GitActionResponse::from(result)))
}

#[cfg(test)]
#[path = "git_sync_tests.rs"]
pub(crate) mod git_sync_tests;

#[cfg(test)]
#[path = "git_push_tests.rs"]
mod git_push_tests;
