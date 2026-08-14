//! Branch listing, switching and lifecycle.
//!
//! The switch path is the reason this module exists separately from the older
//! handlers. Checking out a remote-tracking name such as `origin/feat` with a
//! plain `git checkout` succeeds and leaves HEAD detached — git reports no
//! error, so the panel used to claim the switch worked while the repository
//! was actually sitting on a bare commit. Here a remote name is resolved to
//! the local branch it implies and tracking is set up explicitly.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::types::*;
use crate::web::git::{exec, scope, GitFailure, RefName, Reach};

#[path = "git_branch_list.rs"]
mod git_branch_list;

use git_branch_list::{collect, current_head, remote_names, HeadState};

/// GET /api/git/branches — every local and remote branch with tracking state.
pub async fn git_branches(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &query.repo).await?;
    let dir = repo.path();

    let head = current_head(dir).await?;
    let remotes = remote_names(dir).await?;
    let (local, remote) = collect(dir, &head).await?;

    Ok(Json(GitBranchesResponse {
        current: head.branch().unwrap_or_default().to_string(),
        detached: matches!(head, HeadState::Detached { .. }),
        local,
        remote,
        remotes,
    }))
}

/// POST /api/git/checkout — switch branches.
///
/// A remote-tracking name becomes a local tracking branch rather than a
/// detached HEAD, and a branch already checked out in another worktree is
/// refused with that worktree named, which is friendlier than git's message.
pub async fn git_checkout(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCheckoutRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();
    let requested = RefName::parse(&req.branch)?;

    let remotes = remote_names(dir).await?;
    let target = Target::resolve(dir, requested, &remotes).await?;

    let mut args: Vec<&str> = vec!["checkout"];
    if req.carry_changes {
        args.push("--merge");
    }
    match target {
        Target::Existing(name) => args.push(name.as_str()),
        Target::Track { local, remote } => {
            args.extend_from_slice(&["-b", local, "--track", remote.as_str()]);
        }
    }

    Ok(Json(GitActionResponse::from(
        exec::run(dir, &args, Reach::Local).await?,
    )))
}

/// What `git checkout` should actually be asked to do.
enum Target<'a> {
    /// A local branch that already exists.
    Existing(RefName<'a>),
    /// A remote-tracking ref with no local counterpart yet.
    Track { local: &'a str, remote: RefName<'a> },
}

impl<'a> Target<'a> {
    async fn resolve(
        dir: &std::path::Path,
        requested: RefName<'a>,
        remotes: &[String],
    ) -> WebResult<Self> {
        let Some((_, local)) = requested.split_remote(remotes) else {
            return Ok(Self::Existing(requested));
        };
        // `origin/feat` with a local `feat` already present is a request to
        // switch to that local branch, not to create a second one.
        if branch_exists(dir, local).await? {
            return Ok(Self::Existing(RefName::parse(local)?));
        }
        Ok(Self::Track {
            local,
            remote: requested,
        })
    }
}

async fn branch_exists(dir: &std::path::Path, name: &str) -> WebResult<bool> {
    let full = format!("refs/heads/{name}");
    let outcome = exec::run(
        dir,
        &["show-ref", "--verify", "--quiet", &full],
        Reach::Local,
    )
    .await?;
    Ok(outcome.is_ok())
}

/// POST /api/git/branch/create — create a branch, optionally switching to it.
pub async fn git_branch_create(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitBranchCreateRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();
    let name = RefName::parse(&req.name)?;
    let start = req.start_point.as_deref().map(RefName::parse).transpose()?;

    let mut args: Vec<&str> = if req.checkout {
        vec!["checkout", "-b", name.as_str()]
    } else {
        vec!["branch", name.as_str()]
    };
    if let Some(start) = start {
        args.push(start.as_str());
    }

    Ok(Json(GitActionResponse::from(
        exec::run(dir, &args, Reach::Local).await?,
    )))
}

/// POST /api/git/branch/delete — delete a branch locally or on its remote.
pub async fn git_branch_delete(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitBranchDeleteRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();
    let name = RefName::parse(&req.name)?;

    if let Some(remote) = req.remote.as_deref() {
        let remote = RefName::parse(remote)?;
        let refspec = format!(":{}", name.as_str());
        let args = ["push", remote.as_str(), &refspec];
        return Ok(Json(GitActionResponse::from(
            exec::run(dir, &args, Reach::Network).await?,
        )));
    }

    let head = current_head(dir).await?;
    if head.branch() == Some(name.as_str()) {
        return Ok(Json(GitActionResponse::blocked(
            GitFailure::Failed,
            "You cannot delete the branch you are on. Switch to another branch first.",
        )));
    }

    let flag = if req.force { "-D" } else { "-d" };
    let args = ["branch", flag, name.as_str()];
    Ok(Json(GitActionResponse::from(
        exec::run(dir, &args, Reach::Local).await?,
    )))
}

/// POST /api/git/branch/rename — rename a branch, keeping its upstream.
pub async fn git_branch_rename(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitBranchRenameRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let from = RefName::parse(&req.from)?;
    let to = RefName::parse(&req.to)?;

    let args = ["branch", "-m", from.as_str(), to.as_str()];
    Ok(Json(GitActionResponse::from(
        exec::run(repo.path(), &args, Reach::Local).await?,
    )))
}

#[cfg(test)]
#[path = "git_branch_tests.rs"]
mod git_branch_tests;
