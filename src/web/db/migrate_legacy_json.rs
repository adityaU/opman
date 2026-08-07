//! One-time JSON→SQLite migration for legacy installs.
//!
//! Older versions of opman persisted assistant state to
//! `~/.config/opman/web-assistant-state.json`. On first run against such an
//! install we import the surviving collections and rename the file so the
//! import never repeats.

use std::collections::HashMap;
use std::path::PathBuf;

use tracing::{info, warn};

use super::migrate::run_schema_migrations;
use super::Db;
use crate::web::types::*;

// ── Legacy JSON migration ───────────────────────────────────────────

/// Mirrors the old JSON persistence format.
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
}

/// Attempt to migrate legacy JSON data into the given `Db`.
///
/// This is idempotent: if the DB already has data (memory count > 0)
/// or the JSON file does not exist, it is a no-op.
pub fn run_migration(db: &Db) {
    run_migration_from(db, legacy_json_path());
}

/// Testable core of [`run_migration`]: same logic, but the legacy JSON path is
/// passed in explicitly so tests can point it at a temp file.
fn run_migration_from(db: &Db, json_path: PathBuf) {
    // First, run schema migrations for existing SQLite databases
    run_schema_migrations(db);

    if !json_path.exists() {
        return;
    }

    // Only migrate into an empty database.
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
#[path = "migrate_legacy_json_tests.rs"]
mod migrate_legacy_json_tests;
