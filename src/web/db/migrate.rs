//! Database migrations — schema evolution for existing databases.
//!
//! Also handles one-time JSON→SQLite migration for legacy installs.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::params;
use tracing::{info, warn};

use super::Db;
use crate::web::types::*;

// ── Schema migrations ───────────────────────────────────────────────

/// Run all pending schema migrations.
///
/// Called after `create_tables()` has ensured the base schema exists.
/// Migrations are idempotent — they check column existence before acting.
pub fn run_schema_migrations(db: &Db) {
    migrate_missions_v2(db);
    migrate_routines_v2(db);
}

/// Migrate missions table from v1 (CRUD tracker) to v2 (goal-driven loop).
///
/// v1 columns: id, title, goal, next_action, status, project_index, session_id, created_at, updated_at
/// v2 columns: id, goal, session_id, project_index, state, iteration, max_iterations,
///             last_verdict, last_eval_summary, eval_history, created_at, updated_at
///
/// Strategy: check for the v2 `state` column. If it already exists, we're done.
/// If not, check for the v1 `title` column to extract old data, then drop and
/// recreate the table with v2 schema.
fn migrate_missions_v2(db: &Db) {
    let conn = db.conn();

    // If the `state` column already exists, we're on v2 — nothing to do.
    let has_state = conn.prepare("SELECT state FROM missions LIMIT 0").is_ok();
    if has_state {
        return;
    }

    // Check if old `title` column exists (v1 indicator)
    let has_title = conn.prepare("SELECT title FROM missions LIMIT 0").is_ok();

    if has_title {
        info!("migrating missions table from v1 to v2 (goal-driven loop)");

        // Read existing v1 data
        struct V1Mission {
            id: String,
            title: String,
            goal: String,
            session_id: Option<String>,
            project_index: i64,
            created_at: String,
            updated_at: String,
        }

        let old_missions: Vec<V1Mission> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, title, goal, next_action, status, project_index,
                            session_id, created_at, updated_at
                     FROM missions",
                )
                .expect("read v1 missions");

            let rows: Vec<V1Mission> = stmt
                .query_map([], |row| {
                    Ok(V1Mission {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        goal: row.get(2)?,
                        session_id: row.get(6)?,
                        project_index: row.get(5)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })
                .expect("query v1 missions")
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        // Drop and recreate with v2 schema
        conn.execute_batch(
            "DROP TABLE IF EXISTS missions;
             CREATE TABLE missions (
                id              TEXT PRIMARY KEY,
                goal            TEXT NOT NULL DEFAULT '',
                session_id      TEXT NOT NULL DEFAULT '',
                project_index   INTEGER NOT NULL DEFAULT 0,
                state           TEXT NOT NULL DEFAULT 'pending',
                iteration       INTEGER NOT NULL DEFAULT 0,
                max_iterations  INTEGER NOT NULL DEFAULT 10,
                last_verdict    TEXT,
                last_eval_summary TEXT,
                eval_history    TEXT NOT NULL DEFAULT '[]',
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_missions_state ON missions(state);
             CREATE INDEX IF NOT EXISTS idx_missions_session ON missions(session_id);",
        )
        .expect("recreate missions table v2");

        // Re-insert old data mapped to v2 model
        for m in &old_missions {
            let goal = if m.goal.is_empty() {
                m.title.clone()
            } else {
                format!("{}: {}", m.title, m.goal)
            };
            let session_id = m.session_id.as_deref().unwrap_or("");
            conn.execute(
                "INSERT INTO missions (id, goal, session_id, project_index, state,
                 iteration, max_iterations, eval_history, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending', 0, 10, '[]', ?5, ?6)",
                params![
                    m.id,
                    goal,
                    session_id,
                    m.project_index,
                    m.created_at,
                    m.updated_at
                ],
            )
            .expect("re-insert mission v2");
        }

        info!("migrated {} missions from v1 to v2", old_missions.len());
    } else {
        // Intermediate/unknown schema without `state` — drop and recreate clean.
        // Any existing rows are from an incompatible schema and cannot be preserved.
        warn!("missions table missing `state` column — recreating with v2 schema");
        conn.execute_batch(
            "DROP TABLE IF EXISTS missions;
             CREATE TABLE missions (
                id              TEXT PRIMARY KEY,
                goal            TEXT NOT NULL DEFAULT '',
                session_id      TEXT NOT NULL DEFAULT '',
                project_index   INTEGER NOT NULL DEFAULT 0,
                state           TEXT NOT NULL DEFAULT 'pending',
                iteration       INTEGER NOT NULL DEFAULT 0,
                max_iterations  INTEGER NOT NULL DEFAULT 10,
                last_verdict    TEXT,
                last_eval_summary TEXT,
                eval_history    TEXT NOT NULL DEFAULT '[]',
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_missions_state ON missions(state);
             CREATE INDEX IF NOT EXISTS idx_missions_session ON missions(session_id);",
        )
        .expect("recreate missions table v2 (from intermediate)");
    }
}

// ── Routines v2 migration ───────────────────────────────────────────

/// Migrate routines table from v1 (simple metadata) to v2 (message-dispatch).
///
/// v1 columns: id, name, trigger, action, mission_id, session_id, created_at, updated_at
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
        action: String,
        mission_id: Option<String>,
        session_id: Option<String>,
        created_at: String,
        updated_at: String,
    }

    let old_routines: Vec<V1Routine> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, trigger, action, mission_id,
                        session_id, created_at, updated_at
                 FROM routines",
            )
            .unwrap_or_else(|_| {
                // Table might not exist yet at all
                return conn.prepare("SELECT 1 WHERE 0").unwrap();
            });

        stmt.query_map([], |row| {
            Ok(V1Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger: row.get(2)?,
                action: row.get(3)?,
                mission_id: row.get(4)?,
                session_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
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
        let mut stmt = conn
            .prepare(
                "SELECT id, routine_id, status, summary, created_at
                 FROM routine_runs",
            )
            .unwrap_or_else(|_| conn.prepare("SELECT 1 WHERE 0").unwrap());

        stmt.query_map([], |row| {
            Ok(V1Run {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                status: row.get(2)?,
                summary: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    };

    // Drop and recreate with v2 schema
    conn.execute_batch(
        "DROP TABLE IF EXISTS routines;
         DROP TABLE IF EXISTS routine_runs;
         CREATE TABLE routines (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            trigger       TEXT NOT NULL DEFAULT 'manual',
            action        TEXT NOT NULL DEFAULT 'send_message',
            enabled       INTEGER NOT NULL DEFAULT 1,
            cron_expr     TEXT,
            timezone      TEXT,
            target_mode   TEXT,
            session_id    TEXT,
            project_index INTEGER,
            prompt        TEXT,
            provider_id   TEXT,
            model_id      TEXT,
            mission_id    TEXT,
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
        // Map old action names to v2 (keep legacy actions, default new ones to send_message)
        let action = r.action.as_str();
        // Map old trigger: on_session_idle/daily_summary stay; manual stays
        conn.execute(
            "INSERT INTO routines (id, name, trigger, action, enabled, session_id,
             mission_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)",
            params![
                r.id,
                r.name,
                r.trigger,
                action,
                r.session_id,
                r.mission_id,
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

// ── Legacy JSON migration ───────────────────────────────────────────

/// Mirrors the old JSON persistence format (v1 mission model).
/// We no longer import old missions from JSON since the model has changed.
#[derive(Debug, Clone, serde::Deserialize, Default)]
struct LegacyState {
    #[serde(default)]
    personal_memory: HashMap<String, PersonalMemoryItem>,
    #[serde(default)]
    autonomy_settings: Option<AutonomySettings>,
    #[serde(default)]
    routines: HashMap<String, RoutineDefinition>,
    #[serde(default)]
    routine_runs: Vec<RoutineRunRecord>,
    #[serde(default)]
    delegated_work: HashMap<String, DelegatedWorkItem>,
    #[serde(default)]
    workspaces: HashMap<String, WorkspaceSnapshot>,
    #[serde(default)]
    signals: Vec<SignalInput>,
}

/// Attempt to migrate legacy JSON data into the given `Db`.
///
/// This is idempotent: if the DB already has data (memory count > 0)
/// or the JSON file does not exist, it is a no-op.
///
/// Note: Old missions are NOT migrated from JSON since the model has
/// fundamentally changed. They will be dropped.
pub fn run_migration(db: &Db) {
    // First, run schema migrations for existing SQLite databases
    run_schema_migrations(db);

    let json_path = legacy_json_path();
    if !json_path.exists() {
        return;
    }

    // Only migrate into an empty database (check memory since missions
    // model has changed)
    if !db.list_memory().is_empty() {
        info!("database already has data, skipping JSON migration");
        return;
    }

    let content = match std::fs::read_to_string(&json_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("could not read legacy JSON state: {e}");
            return;
        }
    };

    let state: LegacyState = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            warn!("could not parse legacy JSON state: {e}");
            return;
        }
    };

    let mut count = 0usize;

    // Note: missions are intentionally NOT migrated — the model has changed
    for m in state.personal_memory.values() {
        db.insert_memory(m);
        count += 1;
    }
    if let Some(ref s) = state.autonomy_settings {
        db.save_autonomy_settings(s);
        count += 1;
    }
    for r in state.routines.values() {
        db.insert_routine(r);
        count += 1;
    }
    for r in &state.routine_runs {
        db.insert_routine_run(r);
        count += 1;
    }
    for d in state.delegated_work.values() {
        db.insert_delegated_work(d);
        count += 1;
    }
    for ws in state.workspaces.values() {
        db.upsert_workspace(ws);
        count += 1;
    }
    for s in &state.signals {
        db.insert_signal(s);
        count += 1;
    }

    info!("migrated {count} records from JSON to SQLite (missions skipped — model changed)");

    // Rename the JSON file so migration doesn't repeat
    let backup = json_path.with_extension("json.bak");
    if let Err(e) = std::fs::rename(&json_path, &backup) {
        warn!("could not rename legacy JSON file: {e}");
    }
}

fn legacy_json_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opman")
        .join("web-assistant-state.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrate_empty_db_from_json() {
        let db = Db::open_memory().unwrap();
        // Migration with no JSON file should be a no-op
        run_migration(&db);
        assert!(db.list_missions().is_empty());
    }

    #[test]
    fn schema_migration_idempotent() {
        let db = Db::open_memory().unwrap();
        // Running schema migrations twice should be safe
        run_schema_migrations(&db);
        run_schema_migrations(&db);
        assert!(db.list_missions().is_empty());
    }

    /// Helper: build a Db from a raw Connection that already has a legacy schema.
    fn db_from_raw(conn: Connection) -> Db {
        Db {
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        }
    }

    /// Regression test: an existing v1 missions table (has `title`, no `state`)
    /// should be migrated to v2 without error.
    #[test]
    fn migrate_v1_missions_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

        // Create the v1 missions table (as it existed before v2 redesign)
        conn.execute_batch(
            "CREATE TABLE missions (
                id            TEXT PRIMARY KEY,
                title         TEXT NOT NULL DEFAULT '',
                goal          TEXT NOT NULL DEFAULT '',
                next_action   TEXT NOT NULL DEFAULT '',
                status        TEXT NOT NULL DEFAULT 'active',
                project_index INTEGER NOT NULL DEFAULT 0,
                session_id    TEXT,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            );",
        )
        .unwrap();

        // Insert a v1 mission
        conn.execute(
            "INSERT INTO missions (id, title, goal, next_action, status,
             project_index, created_at, updated_at)
             VALUES ('m1', 'Ship v1', 'Release the first version', 'Write tests',
                     'active', 0, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        // Also create remaining tables so the rest of the code works
        super::super::schema::create_tables(&conn).unwrap();

        let db = db_from_raw(conn);

        // This is the critical call — must not panic
        run_schema_migrations(&db);

        // Indexes must succeed now that `state` column exists
        {
            let conn = db.conn();
            super::super::schema::create_indexes(&conn).unwrap();
        }

        // The old mission should have been migrated
        let missions = db.list_missions();
        assert_eq!(missions.len(), 1);
        assert_eq!(missions[0].id, "m1");
        assert!(missions[0].goal.contains("Ship v1"));
    }

    /// Regression test: a missions table with neither `title` nor `state`
    /// (intermediate/corrupt schema) should be dropped and recreated cleanly.
    #[test]
    fn migrate_intermediate_missions_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

        // Create an intermediate missions table (no `title`, no `state`)
        conn.execute_batch(
            "CREATE TABLE missions (
                id         TEXT PRIMARY KEY,
                goal       TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();

        // Insert a row in the broken schema
        conn.execute(
            "INSERT INTO missions (id, goal, created_at, updated_at)
             VALUES ('m1', 'orphan', '2025-01-01', '2025-01-01')",
            [],
        )
        .unwrap();

        // Create remaining tables
        super::super::schema::create_tables(&conn).unwrap();

        let db = db_from_raw(conn);

        // Must not panic — should drop and recreate
        run_schema_migrations(&db);

        {
            let conn = db.conn();
            super::super::schema::create_indexes(&conn).unwrap();
        }

        // Old incompatible rows are dropped
        assert!(db.list_missions().is_empty());
    }
}
