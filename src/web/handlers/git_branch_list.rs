//! Reading branch state out of git in as few processes as possible.
//!
//! One `for-each-ref` carries name, upstream, ahead/behind, owning worktree,
//! date and subject together, so a repository with two hundred branches still
//! costs two spawns rather than two hundred.

use std::path::Path;

use crate::web::error::WebResult;
use crate::web::git::{exec, Reach};
use crate::web::types::GitBranchInfo;

/// Field order in [`FORMAT`]. Tab-separated, so `splitn` keeps a subject
/// containing a tab from shifting every later column.
const FORMAT: &str = "--format=%(refname:short)%09%(upstream:short)%09%(upstream:track)%09%(worktreepath)%09%(committerdate:iso-strict)%09%(contents:subject)";

const FIELDS: usize = 6;

/// Where HEAD points.
pub enum HeadState {
    /// On a branch.
    Attached(String),
    /// On a bare commit.
    Detached { short: String },
    /// A repository with no commits yet.
    Unborn,
}

impl HeadState {
    pub fn branch(&self) -> Option<&str> {
        match self {
            Self::Attached(name) => Some(name),
            _ => None,
        }
    }
}

/// Resolve HEAD without treating an unborn or detached repository as an error.
///
/// The commit check comes first because `symbolic-ref` succeeds on an unborn
/// repository — HEAD names `refs/heads/main` before that branch exists — and
/// reporting a branch nothing points at would make every later read fail.
pub async fn current_head(dir: &Path) -> WebResult<HeadState> {
    let short = exec::run(
        dir,
        &["rev-parse", "--verify", "--quiet", "--short", "HEAD"],
        Reach::Local,
    )
    .await?;
    let Ok(short) = short else {
        return Ok(HeadState::Unborn);
    };

    let symbolic = exec::run(
        dir,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        Reach::Local,
    )
    .await?;
    Ok(match symbolic {
        Ok(output) => HeadState::Attached(output.trimmed().to_string()),
        Err(_) => HeadState::Detached {
            short: short.trimmed().to_string(),
        },
    })
}

/// Configured remote names, in git's own order.
pub async fn remote_names(dir: &Path) -> WebResult<Vec<String>> {
    let output = exec::run_lenient(dir, &["remote"]).await?;
    Ok(output.lines().map(str::to_string).collect())
}

/// Every local and remote-tracking branch, in that order.
pub async fn collect(dir: &Path, head: &HeadState) -> WebResult<(Vec<GitBranchInfo>, Vec<GitBranchInfo>)> {
    let locals = read(dir, "refs/heads", false, head).await?;
    let remotes = read(dir, "refs/remotes", true, head).await?;
    Ok((locals, remotes))
}

async fn read(
    dir: &Path,
    namespace: &str,
    remote: bool,
    head: &HeadState,
) -> WebResult<Vec<GitBranchInfo>> {
    let output = exec::run_lenient(
        dir,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            FORMAT,
            namespace,
        ],
    )
    .await?;

    Ok(output
        .stdout
        .lines()
        .filter_map(|line| parse(line, remote, head))
        .collect())
}

fn parse(line: &str, remote: bool, head: &HeadState) -> Option<GitBranchInfo> {
    let mut fields = line.splitn(FIELDS, '\t');
    let name = fields.next()?.trim();
    if name.is_empty() || name.ends_with("/HEAD") {
        return None;
    }

    let upstream = fields.next().unwrap_or_default().trim();
    let track = fields.next().unwrap_or_default();
    let worktree = fields.next().unwrap_or_default().trim();
    let date = fields.next().unwrap_or_default().trim();
    let subject = fields.next().unwrap_or_default().trim();

    let (ahead, behind) = parse_track(track);

    Some(GitBranchInfo {
        name: name.to_string(),
        current: !remote && head.branch() == Some(name),
        remote,
        upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
        ahead,
        behind,
        subject: subject.to_string(),
        date: date.to_string(),
        worktree: (!worktree.is_empty()).then(|| worktree.to_string()),
    })
}

/// Read `[ahead 3, behind 1]` — git's own wording — into a pair.
///
/// `[gone]` means the upstream was deleted; both counts are zero and the
/// missing upstream is what the UI keys off, so nothing special is needed.
fn parse_track(track: &str) -> (u32, u32) {
    let count_after = |keyword: &str| {
        track
            .split(|c: char| !c.is_ascii_alphanumeric())
            .skip_while(|token| *token != keyword)
            .nth(1)
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    (count_after("ahead"), count_after("behind"))
}

#[cfg(test)]
#[path = "git_branch_list_tests.rs"]
mod git_branch_list_tests;
