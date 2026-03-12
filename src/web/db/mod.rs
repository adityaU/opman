//! SQLite persistence layer for the web assistant state.
//!
//! Replaces the previous JSON-file persistence with a proper database.
//! Uses a single `rusqlite::Connection` wrapped in a `Mutex` for
//! thread-safe access from async handlers (via `spawn_blocking`).

mod delegation;
mod memory;
pub(crate) mod migrate;
mod missions;
mod routines;
mod schema;
mod settings;
mod signals;
mod workspaces;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tracing::info;

/// Thread-safe handle to the SQLite database.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// Open (or create) the assistant database at the standard config path.
    pub fn open() -> anyhow::Result<Self> {
        let path = db_path();
        Self::open_at(path)
    }

    /// Open (or create) a database at an explicit path.
    pub fn open_at(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        schema::create_tables(&conn)?;

        info!("opened assistant database at {}", path.display());

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Run schema migrations before creating indexes — migrations may
        // drop/recreate tables to add missing columns (e.g. `state` on
        // missions), so indexes that reference those columns must come after.
        migrate::run_schema_migrations(&db);

        {
            let conn = db.conn();
            schema::create_indexes(&conn)?;
        }

        Ok(db)
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn open_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        schema::create_tables(&conn)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        migrate::run_schema_migrations(&db);
        {
            let conn = db.conn();
            schema::create_indexes(&conn)?;
        }
        Ok(db)
    }

    /// Acquire the connection lock. All public CRUD methods use this.
    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("db mutex poisoned")
    }
}

/// Standard path: `~/.config/opman/assistant.db`
fn db_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opman")
        .join("assistant.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_memory_succeeds() {
        let db = Db::open_memory().unwrap();
        // Verify tables exist by querying sqlite_master
        let conn = db.conn();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // We expect at least 7 tables
        assert!(count >= 7, "expected >=7 tables, got {}", count);
    }
}
