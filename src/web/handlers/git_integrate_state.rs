//! Probing which multi-step git operation a repository is in the middle of.
//!
//! The markers all live in the git directory, which is *not* `.git` inside a
//! linked worktree — there `.git` is a file and the real directory is
//! `…/.git/worktrees/<name>`. Every lookup therefore goes through
//! `git rev-parse --git-path`, which resolves the right one in either case.

use std::path::{Path, PathBuf};

use crate::web::error::WebResult;
use crate::web::git::exec::run_lenient;
use crate::web::types::{GitOperationKind, GitOperationResponse};

/// Single-file markers, in the order they are checked. A rebase is probed
/// first because it leaves `CHERRY_PICK_HEAD` behind while replaying.
const MARKERS: [(&str, GitOperationKind); 4] = [
    ("MERGE_HEAD", GitOperationKind::Merge),
    ("CHERRY_PICK_HEAD", GitOperationKind::CherryPick),
    ("REVERT_HEAD", GitOperationKind::Revert),
    ("BISECT_LOG", GitOperationKind::Bisect),
];

/// Resolve `name` inside the git directory, yielding it only when it exists.
async fn git_path(dir: &Path, name: &str) -> WebResult<Option<PathBuf>> {
    let output = run_lenient(dir, &["rev-parse", "--git-path", name]).await?;
    let raw = output.trimmed();
    if raw.is_empty() {
        return Ok(None);
    }
    // `join` with an absolute path yields that path, so this handles both the
    // relative form git prints in a normal repository and the absolute form.
    let path = dir.join(raw);
    Ok(path.exists().then_some(path))
}

/// Trimmed contents of a one-line control file, absent when unreadable.
async fn read_line(dir: &Path, name: &str) -> Option<String> {
    let text = tokio::fs::read_to_string(dir.join(name)).await.ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Same, parsed as a counter.
async fn read_number(dir: &Path, name: &str) -> Option<u32> {
    read_line(dir, name).await?.parse().ok()
}

/// Paths git reports as having unresolved conflicts.
async fn conflicted(dir: &Path) -> WebResult<Vec<String>> {
    let output = run_lenient(dir, &["diff", "--name-only", "--diff-filter=U"]).await?;
    Ok(output.lines().map(str::to_string).collect())
}

/// What, if anything, is in flight — plus the conflicts either way.
pub(super) async fn probe(dir: &Path) -> WebResult<GitOperationResponse> {
    let mut response = GitOperationResponse {
        kind: None,
        conflicted: conflicted(dir).await?,
        step: None,
        total: None,
        onto: None,
    };

    for name in ["rebase-merge", "rebase-apply"] {
        let Some(state_dir) = git_path(dir, name).await? else {
            continue;
        };
        response.kind = Some(GitOperationKind::Rebase);
        response.step = read_number(&state_dir, "msgnum").await;
        response.total = read_number(&state_dir, "end").await;
        response.onto = read_line(&state_dir, "onto").await;
        return Ok(response);
    }

    for (name, kind) in MARKERS {
        if git_path(dir, name).await?.is_some() {
            response.kind = Some(kind);
            return Ok(response);
        }
    }

    Ok(response)
}
