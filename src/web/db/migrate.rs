//! One-time migration: JSON file → SQLite.
//!
//! If the legacy `web-assistant-state.json` exists and the database is
//! empty, we import its contents into SQLite and rename the JSON file
//! to `.json.bak` so the migration is not repeated.

use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{info, warn};

use super::Db;
use crate::web::types::*;

/// Mirrors the old JSON persistence format.
#[derive(Debug, Clone, serde::Deserialize, Default)]
struct LegacyState {
    #[serde(default)]
    missions: HashMap<String, Mission>,
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
/// This is idempotent: if the DB already has data (missions count > 0)
/// or the JSON file does not exist, it is a no-op.
pub fn run_migration(db: &Db) {
    let json_path = legacy_json_path();
    if !json_path.exists() {
        return;
    }

    // Only migrate into an empty database
    if !db.list_missions().is_empty() {
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

    for m in state.missions.values() {
        db.insert_mission(m);
        count += 1;
    }
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

    info!("migrated {count} records from JSON to SQLite");

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
}
