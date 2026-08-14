//! History integration: merge, rebase, reset, revert, cherry-pick, and the
//! continue/abort/skip controls for whichever of them is in flight.
//!
//! Every command is driven with `-c core.editor=true` so git can never stop to
//! open an editor: opman runs headless, and a blocked editor would wedge the
//! request until the deadline in [`crate::web::git::exec`] fired.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::git::exec::{run, Reach};
use super::super::git::{scope, CommitHash, GitFailure, RefName, Revision};
use super::super::types::*;

#[path = "git_integrate_state.rs"]
mod git_integrate_state;

use git_integrate_state::probe;

/// Prefix that makes any git invocation non-interactive without touching the
/// environment: `true` succeeds silently in place of an editor.
const NO_EDITOR: [&str; 2] = ["-c", "core.editor=true"];

/// Human name for an operation, used only in refusal messages.
const fn label(kind: GitOperationKind) -> &'static str {
    match kind {
        GitOperationKind::Merge => "merge",
        GitOperationKind::Rebase => "rebase",
        GitOperationKind::CherryPick => "cherry-pick",
        GitOperationKind::Revert => "revert",
        GitOperationKind::Bisect => "bisect",
    }
}

/// The argv tail for one operation × action pair, or `None` when git has no
/// such control — a merge cannot skip a commit, and a bisect cannot continue.
const fn action_args(
    kind: GitOperationKind,
    action: GitOperationAction,
) -> Option<&'static [&'static str]> {
    use GitOperationAction::{Abort, Continue, Skip};
    use GitOperationKind::{Bisect, CherryPick, Merge, Rebase, Revert};

    Some(match (kind, action) {
        // `git merge --continue` takes no other arguments, so its
        // non-interactivity comes from `core.editor` alone.
        (Merge, Continue) => &["merge", "--continue"],
        (Merge, Abort) => &["merge", "--abort"],
        (Merge, Skip) => return None,
        (Rebase, Continue) => &["rebase", "--continue"],
        (Rebase, Abort) => &["rebase", "--abort"],
        (Rebase, Skip) => &["rebase", "--skip"],
        (CherryPick, Continue) => &["cherry-pick", "--continue"],
        (CherryPick, Abort) => &["cherry-pick", "--abort"],
        (CherryPick, Skip) => &["cherry-pick", "--skip"],
        (Revert, Continue) => &["revert", "--continue", "--no-edit"],
        (Revert, Abort) => &["revert", "--abort"],
        (Revert, Skip) => &["revert", "--skip"],
        (Bisect, Continue) => return None,
        (Bisect, Abort) => &["bisect", "reset"],
        (Bisect, Skip) => &["bisect", "skip"],
    })
}

/// GET /api/git/operation?repo=... — which multi-step operation is in flight.
pub async fn git_operation_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRepoScope>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &query.repo).await?;
    Ok(Json(probe(repo.path()).await?))
}

/// POST /api/git/operation — continue, abort or skip the operation in flight.
pub async fn git_operation(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitOperationRequest>,
) -> WebResult<impl IntoResponse> {
    let repo = scope::resolve(&state, &req.repo).await?;
    let dir = repo.path();

    let Some(kind) = probe(dir).await?.kind else {
        return Ok(Json(GitActionResponse::blocked(
            GitFailure::Failed,
            "No merge, rebase, cherry-pick or revert is in progress.",
        )));
    };
    let Some(tail) = action_args(kind, req.action) else {
        return Ok(Json(GitActionResponse::blocked(
            GitFailure::Failed,
            format!("A {} cannot be skipped or continued that way.", label(kind)),
        )));
    };

    let args: Vec<&str> = NO_EDITOR.iter().chain(tail).copied().collect();
    Ok(Json(run(dir, &args, Reach::Local).await?.into()))
}

/// POST /api/git/merge — merge a branch into the current one.
pub async fn git_merge(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitMergeRequest>,
) -> WebResult<impl IntoResponse> {
    let branch = RefName::parse(&req.branch)?;
    let repo = scope::resolve(&state, &req.repo).await?;

    let mut args: Vec<&str> = NO_EDITOR.to_vec();
    args.extend(["merge", "--no-edit"]);
    if req.no_ff {
        args.push("--no-ff");
    }
    if req.no_commit {
        args.push("--no-commit");
    }
    args.push(branch.as_str());

    Ok(Json::<GitActionResponse>(
        run(repo.path(), &args, Reach::Local).await?.into(),
    ))
}

/// POST /api/git/rebase — replay the current branch onto another.
pub async fn git_rebase(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitRebaseRequest>,
) -> WebResult<impl IntoResponse> {
    let onto = RefName::parse(&req.onto)?;
    let repo = scope::resolve(&state, &req.repo).await?;

    let mut args: Vec<&str> = NO_EDITOR.to_vec();
    args.extend(["rebase", onto.as_str()]);

    Ok(Json::<GitActionResponse>(
        run(repo.path(), &args, Reach::Local).await?.into(),
    ))
}

/// POST /api/git/reset — move HEAD, with the index and tree following as far
/// as the requested mode says.
pub async fn git_reset(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitResetRequest>,
) -> WebResult<impl IntoResponse> {
    // `HEAD~1` is the commonest reset target of all, so this takes a revision
    // expression rather than a bare ref name — the latter rejects `~` because
    // no *branch* may contain one.
    let target = Revision::parse(&req.target)?;
    let repo = scope::resolve(&state, &req.repo).await?;

    let args = ["reset", req.mode.flag(), target.as_str()];
    Ok(Json::<GitActionResponse>(
        run(repo.path(), &args, Reach::Local).await?.into(),
    ))
}

/// POST /api/git/revert — record a commit that undoes another.
pub async fn git_revert(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitReplayRequest>,
) -> WebResult<impl IntoResponse> {
    let hash = CommitHash::parse(&req.hash)?;
    let repo = scope::resolve(&state, &req.repo).await?;

    let mut args: Vec<&str> = NO_EDITOR.to_vec();
    args.extend(["revert", "--no-edit"]);
    if req.no_commit {
        args.push("--no-commit");
    }
    args.push(hash.as_str());

    Ok(Json::<GitActionResponse>(
        run(repo.path(), &args, Reach::Local).await?.into(),
    ))
}

/// POST /api/git/cherry-pick — apply one commit onto the current branch.
pub async fn git_cherry_pick(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitReplayRequest>,
) -> WebResult<impl IntoResponse> {
    let hash = CommitHash::parse(&req.hash)?;
    let repo = scope::resolve(&state, &req.repo).await?;

    let mut args: Vec<&str> = NO_EDITOR.to_vec();
    args.push("cherry-pick");
    if req.no_commit {
        args.push("--no-commit");
    }
    args.push(hash.as_str());

    Ok(Json::<GitActionResponse>(
        run(repo.path(), &args, Reach::Local).await?.into(),
    ))
}

#[cfg(test)]
#[path = "git_integrate_tests.rs"]
pub(crate) mod git_integrate_tests;

#[cfg(test)]
#[path = "git_replay_tests.rs"]
mod git_replay_tests;

#[cfg(test)]
#[path = "git_merge_tests.rs"]
mod git_merge_tests;

#[cfg(test)]
#[path = "git_operation_tests.rs"]
mod git_operation_tests;
