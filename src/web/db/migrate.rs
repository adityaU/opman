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
}

/// Migrate missions table from v1 (CRUD tracker) to v2 (goal-driven loop).
///
/// v1 columns: id, title, goal, next_action, status, project_index, session_id, created_at, updated_at
/// v2 columns: id, goal, session_id, project_index, state, iteration, max_iterations,
///             last_verdict, last_eval_summary, eval_history, created_at, updated_at
///
/// Strategy: check for old `title` column. If it exists, we have v1 data.
/// Recreate the table with v2 schema, migrating any existing rows.
fn migrate_missions_v2(db: &Db) {
    let conn = db.conn();

    // Check if old `title` column exists (v1 indicator)
    let has_title = conn.prepare("SELECT title FROM missions LIMIT 0").is_ok();

    if !has_title {
        // Already on v2 schema or fresh install — nothing to do
        return;
    }

    info!("migrating missions table from v1 to v2 (goal-driven loop)");

    // Read existing v1 data
    let mut stmt = conn
        .prepare(
            "SELECT id, title, goal, next_action, status, project_index,
                    session_id, created_at, updated_at
             FROM missions",
        )
        .expect("read v1 missions");

    struct V1Mission {
        id: String,
        title: String,
        goal: String,
        session_id: Option<String>,
        project_index: i64,
        created_at: String,
        updated_at: String,
    }

    let old_missions: Vec<V1Mission> = stmt
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
    drop(stmt);

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
    // Goal = old title + goal combined; state = pending
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
}
