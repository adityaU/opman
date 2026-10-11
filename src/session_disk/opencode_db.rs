//! opencode's SQLite store (`opencode.db`, opencode 1.x).
//!
//! Opened read-only and never written: opencode may be running against the same file. WAL
//! mode lets a reader see committed rows while opencode writes, so no snapshot copy is needed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use rusqlite::{params, Connection, OpenFlags};

use crate::app::{SessionInfo, SessionTime};

/// The default channel's database name; other release channels write `opencode-<channel>.db`.
const DEFAULT_DB: &str = "opencode.db";

/// The database under opencode's data directory, if this install uses one.
pub(super) fn database(root: &Path) -> Option<PathBuf> {
    let default = root.join(DEFAULT_DB);
    if default.is_file() {
        return Some(default);
    }
    // A non-default channel: the most recently written one is the install in use.
    std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("opencode-") && name.ends_with(".db")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// The sessions opencode's `GET /session` would return for `directory`.
///
/// opencode lists the *project's* most recently updated sessions — the project the
/// directory resolves to — and opman then keeps those in the directory itself. Mirroring both
/// steps means the live list that later replaces this one has the same rows.
pub(super) fn list(db: &Path, directory: &str, limit: usize) -> anyhow::Result<Vec<SessionInfo>> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(db, flags)
        .with_context(|| format!("opening {}", db.display()))?;
    conn.busy_timeout(Duration::from_millis(250))?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut query = conn.prepare(
        "SELECT id, title, parent_id, directory, time_created, time_updated
           FROM session
          WHERE project_id = (SELECT project_id FROM session
                               WHERE directory = ?1
                               ORDER BY time_updated DESC LIMIT 1)
          ORDER BY time_updated DESC, id DESC
          LIMIT ?2",
    )?;
    let rows = query.query_map(params![directory, limit], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            parent_id: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            directory: row.get(3)?,
            time: SessionTime {
                created: millis(row.get(4)?),
                updated: millis(row.get(5)?),
            },
            engine: crate::app::EngineChoices::default(),
        })
    })?;
    let mut found = Vec::new();
    for row in rows {
        let session = row?;
        if session.directory == directory {
            found.push(session);
        }
    }
    Ok(found)
}

fn millis(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

#[cfg(test)]
#[path = "opencode_db_tests.rs"]
mod opencode_db_tests;
