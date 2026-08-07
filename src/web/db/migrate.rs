//! Database migrations — schema evolution for existing databases.
//!
//! The one-time JSON→SQLite import for legacy installs lives in
//! [`super::migrate_legacy_json`].

use rusqlite::params;
use tracing::{info, warn};

use super::Db;

// ── Schema migrations ───────────────────────────────────────────────

/// Run all pending schema migrations.
///
/// Called after `create_tables()` has ensured the base schema exists.
/// Migrations are idempotent — they check column existence before acting.
pub fn run_schema_migrations(db: &Db) {
    migrate_drop_removed_features(db);
    migrate_routines_v2(db);
    migrate_kanban_archived(db);
}

/// Drop the tables and indexes of features that no longer exist: missions,
/// delegated work, workspace snapshots, and signals.
///
/// Idempotent by construction — `IF EXISTS` needs no version bookkeeping.
/// Runs before [`migrate_routines_v2`], which is safe: that migration reads
/// `mission_id` from `routines`, never from `missions`.
fn migrate_drop_removed_features(db: &Db) {
    let conn = db.conn();
    if let Err(e) = conn.execute_batch(
        "DROP INDEX IF EXISTS idx_missions_state;
         DROP INDEX IF EXISTS idx_missions_session;
         DROP INDEX IF EXISTS idx_delegated_status;
         DROP INDEX IF EXISTS idx_signals_created;
         DROP TABLE IF EXISTS missions;
         DROP TABLE IF EXISTS delegated_work;
         DROP TABLE IF EXISTS workspaces;
         DROP TABLE IF EXISTS signals;",
    ) {
        warn!("could not drop removed-feature tables: {e}");
    }
}

/// Add the `archived` column to `kanban_tasks` for databases created before the
/// archive feature. Idempotent — checks for the column first.
fn migrate_kanban_archived(db: &Db) {
    let conn = db.conn();
    let has_archived = conn
        .prepare("SELECT archived FROM kanban_tasks LIMIT 0")
        .is_ok();
    if has_archived {
        return;
    }
    info!("adding `archived` column to kanban_tasks");
    let _ = conn.execute(
        "ALTER TABLE kanban_tasks ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
        [],
    );
}

// ── Routines v2 migration ───────────────────────────────────────────

/// Migrate routines table from v1 (simple metadata) to v2 (message-dispatch).
///
/// v1 columns: id, name, trigger, action, mission_id, session_id, created_at, updated_at
/// (the `action` and `mission_id` columns are dropped rather than carried over)
/// v2 adds: enabled, cron_expr, timezone, target_mode, project_index, prompt,
///          provider_id, model_id, last_run_at, next_run_at, last_error
///
/// Also migrates routine_runs to add target_session_id, duration_ms.
///
/// Strategy: check for the v2 `enabled` column. If it exists, done.
/// If not, drop and recreate with v2 schema, preserving data.
fn migrate_routines_v2(db: &Db) {
    let conn = db.conn();

    // If `enabled` column already exists, we're on v2.
    let has_enabled = conn.prepare("SELECT enabled FROM routines LIMIT 0").is_ok();
    if has_enabled {
        return;
    }

    info!("migrating routines table from v1 to v2 (message-dispatch automation)");

    // Read existing v1 routines
    struct V1Routine {
        id: String,
        name: String,
        trigger: String,
        session_id: Option<String>,
        created_at: String,
        updated_at: String,
    }

    let old_routines: Vec<V1Routine> = {
        match conn.prepare(
            "SELECT id, name, trigger, session_id, created_at, updated_at
             FROM routines",
        ) {
            Ok(mut stmt) => match stmt.query_map([], |row| {
                Ok(V1Routine {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    trigger: row.get(2)?,
                    session_id: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            }) {
                Ok(rows) => rows.filter_map(Result::ok).collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    };

    // Read existing v1 routine runs
    struct V1Run {
        id: String,
        routine_id: String,
        status: String,
        summary: String,
        created_at: String,
    }

    let old_runs: Vec<V1Run> = {
        match conn.prepare(
            "SELECT id, routine_id, status, summary, created_at
             FROM routine_runs",
        ) {
            Ok(mut stmt) => match stmt.query_map([], |row| {
                Ok(V1Run {
                    id: row.get(0)?,
                    routine_id: row.get(1)?,
                    status: row.get(2)?,
                    summary: row.get(3)?,
                    created_at: row.get(4)?,
                })
            }) {
                Ok(rows) => rows.filter_map(Result::ok).collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    };

    // Drop and recreate with v2 schema
    conn.execute_batch(
        "DROP TABLE IF EXISTS routines;
         DROP TABLE IF EXISTS routine_runs;
         CREATE TABLE routines (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            trigger       TEXT NOT NULL DEFAULT 'manual',
            enabled       INTEGER NOT NULL DEFAULT 1,
            cron_expr     TEXT,
            timezone      TEXT,
            target_mode   TEXT,
            session_id    TEXT,
            project_index INTEGER,
            prompt        TEXT,
            provider_id   TEXT,
            model_id      TEXT,
            last_run_at   TEXT,
            next_run_at   TEXT,
            last_error    TEXT,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
         );
         CREATE TABLE routine_runs (
            id                TEXT PRIMARY KEY,
            routine_id        TEXT NOT NULL,
            status            TEXT NOT NULL DEFAULT 'completed',
            summary           TEXT NOT NULL DEFAULT '',
            target_session_id TEXT,
            duration_ms       INTEGER,
            created_at        TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_routine_runs_routine ON routine_runs(routine_id);",
    )
    .expect("recreate routines/runs tables v2");

    // Re-insert old routines mapped to v2 model
    for r in &old_routines {
        // Old trigger names carry over unchanged; the dead `action` and
        // `mission_id` columns are dropped.
        conn.execute(
            "INSERT INTO routines (id, name, trigger, enabled, session_id,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
            params![
                r.id,
                r.name,
                r.trigger,
                r.session_id,
                r.created_at,
                r.updated_at
            ],
        )
        .expect("re-insert routine v2");
    }

    // Re-insert old runs
    for run in &old_runs {
        conn.execute(
            "INSERT INTO routine_runs (id, routine_id, status, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run.id,
                run.routine_id,
                run.status,
                run.summary,
                run.created_at
            ],
        )
        .expect("re-insert routine_run v2");
    }

    info!(
        "migrated {} routines and {} runs from v1 to v2",
        old_routines.len(),
        old_runs.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_migration_idempotent() {
        let db = Db::open_memory().unwrap();
        // Running schema migrations twice should be safe
        run_schema_migrations(&db);
        run_schema_migrations(&db);
        assert!(db.list_routines().is_empty());
    }

    /// The drop migration must remove legacy tables from an existing database
    /// and stay silent when they are already gone.
    #[test]
    fn drop_removed_features_is_idempotent() {
        let db = Db::open_memory().unwrap();
        {
            let conn = db.conn();
            conn.execute_batch(
                "CREATE TABLE missions (id TEXT PRIMARY KEY);
                 CREATE TABLE delegated_work (id TEXT PRIMARY KEY);
                 CREATE TABLE workspaces (name TEXT PRIMARY KEY);
                 CREATE TABLE signals (id TEXT PRIMARY KEY);",
            )
            .unwrap();
        }

        migrate_drop_removed_features(&db);
        migrate_drop_removed_features(&db);

        let conn = db.conn();
        for table in ["missions", "delegated_work", "workspaces", "signals"] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 0, "{table} should have been dropped");
        }
    }
}

#[cfg(test)]
#[cfg(test)]
#[path = "migrate_tests.rs"]
mod migrate_tests;
