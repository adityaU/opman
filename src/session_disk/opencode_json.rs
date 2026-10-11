//! opencode's JSON storage layout, used before it moved to SQLite.
//!
//! `storage/project/<projectID>.json` records each project's `worktree`;
//! `storage/session/<projectID>/<sessionID>.json` holds one session's metadata. Only the
//! projects whose worktree contains the directory are read, and within each only the most
//! recently written files, so a large history costs a directory listing, not a full parse.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

use crate::app::{SessionInfo, SessionTime};

/// opencode's project for directories outside any git repository.
const GLOBAL_PROJECT: &str = "global";

#[derive(Deserialize)]
struct StoredProject {
    id: String,
    #[serde(default)]
    worktree: String,
}

#[derive(Deserialize)]
struct StoredSession {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "parentID")]
    parent_id: Option<String>,
    #[serde(default)]
    directory: String,
    #[serde(default)]
    time: SessionTime,
}

/// The `limit` most recently updated sessions recorded for `directory`.
pub(super) fn list(
    storage: &Path,
    directory: &str,
    limit: usize,
) -> anyhow::Result<Vec<SessionInfo>> {
    let mut found = Vec::new();
    for project in projects_containing(storage, Path::new(directory)) {
        // The project's `limit` newest sessions, like opencode's own list; a file that is
        // not a session does not take a slot.
        let sessions = newest_files(&storage.join("session").join(project))
            .into_iter()
            .filter_map(|path| read_json::<StoredSession>(&path))
            .take(limit);
        for session in sessions {
            if session.directory == directory {
                found.push(SessionInfo {
                    id: session.id,
                    title: session.title,
                    parent_id: session.parent_id.unwrap_or_default(),
                    directory: session.directory,
                    time: session.time,
                    engine: crate::app::EngineChoices::default(),
                });
            }
        }
    }
    found.sort_by(|a, b| b.time.updated.cmp(&a.time.updated));
    found.truncate(limit);
    Ok(found)
}

/// Ids of the projects whose worktree is `directory` or one of its ancestors.
fn projects_containing(storage: &Path, directory: &Path) -> Vec<String> {
    let mut ids: Vec<String> = json_files(&storage.join("project"))
        .filter_map(|(_, path)| read_json::<StoredProject>(&path))
        .filter(|project| {
            project.id != GLOBAL_PROJECT
                && !project.worktree.is_empty()
                && directory.starts_with(&project.worktree)
        })
        .map(|project| project.id)
        .collect();
    ids.push(GLOBAL_PROJECT.to_string());
    ids
}

/// The `.json` files in `dir`, most recently modified first. Only names and mtimes are
/// read here; the caller parses as few as it needs.
fn newest_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<(SystemTime, PathBuf)> = json_files(dir).collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().map(|(_, path)| path).collect()
}

fn json_files(dir: &Path) -> impl Iterator<Item = (SystemTime, PathBuf)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
}

/// One file's contents; an unreadable or foreign file is skipped rather than fatal.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
#[path = "opencode_json_tests.rs"]
mod opencode_json_tests;
