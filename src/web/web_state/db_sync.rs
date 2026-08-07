//! Full-snapshot sync: in-memory state → SQLite.
//!
//! Called by the debounced persist worker in `background.rs`.
//! Uses the raw `rusqlite::Connection` (single lock acquisition) to
//! avoid deadlocking on the Mutex that the CRUD methods also use.

use rusqlite::params;

use super::super::db::Db;
use super::super::types::*;

/// Replace all DB rows with the provided in-memory snapshot.
///
/// Runs inside a single SQLite transaction for atomicity.
pub(super) fn sync_all(
    db: &Db,
    memory: &[PersonalMemoryItem],
    autonomy: &AutonomySettings,
    routines: &[RoutineDefinition],
    routine_runs: &[RoutineRunRecord],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = db.conn();
    conn.execute_batch("BEGIN IMMEDIATE")?;

    // Clear all tables
    conn.execute("DELETE FROM personal_memory", [])?;
    conn.execute("DELETE FROM routines", [])?;
    conn.execute("DELETE FROM routine_runs", [])?;

    for m in memory {
        conn.execute(
            "INSERT INTO personal_memory (id,label,content,scope,project_index,\
             session_id,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                m.id,
                m.label,
                m.content,
                scope_str(&m.scope),
                m.project_index.map(|v| v as i64),
                m.session_id,
                m.created_at,
                m.updated_at
            ],
        )?;
    }
    conn.execute(
        "INSERT INTO autonomy_settings (id,mode,updated_at) VALUES (1,?1,?2) \
         ON CONFLICT(id) DO UPDATE SET mode=excluded.mode, updated_at=excluded.updated_at",
        params![mode_str(&autonomy.mode), autonomy.updated_at],
    )?;
    for r in routines {
        conn.execute(
            "INSERT INTO routines (id,name,trigger,enabled,cron_expr,timezone,\
             target_mode,session_id,project_index,prompt,provider_id,model_id,\
             last_run_at,next_run_at,last_error,\
             created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                r.id,
                r.name,
                trigger_str(&r.trigger),
                r.enabled as i64,
                r.cron_expr,
                r.timezone,
                r.target_mode.as_ref().map(target_mode_str),
                r.session_id,
                r.project_index.map(|v| v as i64),
                r.prompt,
                r.provider_id,
                r.model_id,
                r.last_run_at,
                r.next_run_at,
                r.last_error,
                r.created_at,
                r.updated_at
            ],
        )?;
    }
    for r in routine_runs {
        conn.execute(
            "INSERT INTO routine_runs (id,routine_id,status,summary,\
             target_session_id,duration_ms,created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                r.id,
                r.routine_id,
                r.status,
                r.summary,
                r.target_session_id,
                r.duration_ms.map(|v| v as i64),
                r.created_at
            ],
        )?;
    }
    conn.execute_batch("COMMIT")?;
    Ok(())
}

// ── String conversion helpers ───────────────────────────────────────

fn scope_str(s: &MemoryScope) -> &'static str {
    match s {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
        MemoryScope::Session => "session",
    }
}

fn mode_str(m: &AutonomyMode) -> &'static str {
    match m {
        AutonomyMode::Observe => "observe",
        AutonomyMode::Nudge => "nudge",
        AutonomyMode::Continue => "continue",
        AutonomyMode::Autonomous => "autonomous",
    }
}

fn trigger_str(t: &RoutineTrigger) -> &'static str {
    match t {
        RoutineTrigger::Manual => "manual",
        RoutineTrigger::Scheduled => "scheduled",
        RoutineTrigger::OnSessionIdle => "on_session_idle",
        RoutineTrigger::DailySummary => "daily_summary",
    }
}

fn target_mode_str(t: &RoutineTargetMode) -> &'static str {
    match t {
        RoutineTargetMode::ExistingSession => "existing_session",
        RoutineTargetMode::NewSession => "new_session",
    }
}

#[cfg(test)]
#[path = "db_sync_tests.rs"]
mod db_sync_tests;
