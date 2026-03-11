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
    missions: &[Mission],
    memory: &[PersonalMemoryItem],
    autonomy: &AutonomySettings,
    routines: &[RoutineDefinition],
    routine_runs: &[RoutineRunRecord],
    delegated: &[DelegatedWorkItem],
    workspaces: &[WorkspaceSnapshot],
    signals: &[SignalInput],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = db.conn();
    conn.execute_batch("BEGIN IMMEDIATE")?;

    // Clear all tables
    conn.execute("DELETE FROM missions", [])?;
    conn.execute("DELETE FROM personal_memory", [])?;
    conn.execute("DELETE FROM routines", [])?;
    conn.execute("DELETE FROM routine_runs", [])?;
    conn.execute("DELETE FROM delegated_work", [])?;
    conn.execute("DELETE FROM workspaces", [])?;
    conn.execute("DELETE FROM signals", [])?;

    for m in missions {
        let eval_history_json =
            serde_json::to_string(&m.eval_history).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO missions (id,goal,session_id,project_index,state,\
             iteration,max_iterations,last_verdict,last_eval_summary,\
             eval_history,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                m.id,
                m.goal,
                m.session_id,
                m.project_index as i64,
                state_str(&m.state),
                m.iteration as i64,
                m.max_iterations as i64,
                m.last_verdict.as_ref().map(verdict_str),
                m.last_eval_summary,
                eval_history_json,
                m.created_at,
                m.updated_at
            ],
        )?;
    }
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
            "INSERT INTO routines (id,name,trigger,action,mission_id,\
             session_id,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                r.id,
                r.name,
                trigger_str(&r.trigger),
                action_str(&r.action),
                r.mission_id,
                r.session_id,
                r.created_at,
                r.updated_at
            ],
        )?;
    }
    for r in routine_runs {
        conn.execute(
            "INSERT INTO routine_runs (id,routine_id,status,summary,created_at) \
             VALUES (?1,?2,?3,?4,?5)",
            params![r.id, r.routine_id, r.status, r.summary, r.created_at],
        )?;
    }
    for d in delegated {
        conn.execute(
            "INSERT INTO delegated_work (id,title,assignee,scope,status,mission_id,\
             session_id,subagent_session_id,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                d.id,
                d.title,
                d.assignee,
                d.scope,
                deleg_str(&d.status),
                d.mission_id,
                d.session_id,
                d.subagent_session_id,
                d.created_at,
                d.updated_at
            ],
        )?;
    }
    for ws in workspaces {
        let json = serde_json::to_string(ws)?;
        conn.execute(
            "INSERT INTO workspaces (name,snapshot,created_at) VALUES (?1,?2,?3)",
            params![ws.name, json, ws.created_at],
        )?;
    }
    for s in signals {
        conn.execute(
            "INSERT INTO signals (id,kind,title,body,created_at,session_id) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![s.id, s.kind, s.title, s.body, s.created_at, s.session_id],
        )?;
    }

    conn.execute_batch("COMMIT")?;
    Ok(())
}

// ── String conversion helpers ───────────────────────────────────────

fn state_str(s: &MissionState) -> &'static str {
    match s {
        MissionState::Pending => "pending",
        MissionState::Executing => "executing",
        MissionState::Evaluating => "evaluating",
        MissionState::Paused => "paused",
        MissionState::Completed => "completed",
        MissionState::Cancelled => "cancelled",
        MissionState::Failed => "failed",
    }
}

fn verdict_str(v: &EvalVerdict) -> &'static str {
    match v {
        EvalVerdict::Achieved => "achieved",
        EvalVerdict::Continue => "continue",
        EvalVerdict::Blocked => "blocked",
        EvalVerdict::Failed => "failed",
    }
}

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
        RoutineTrigger::OnSessionIdle => "on_session_idle",
        RoutineTrigger::DailySummary => "daily_summary",
    }
}

fn action_str(a: &RoutineAction) -> &'static str {
    match a {
        RoutineAction::ReviewMission => "review_mission",
        RoutineAction::OpenInbox => "open_inbox",
        RoutineAction::OpenActivityFeed => "open_activity_feed",
    }
}

fn deleg_str(s: &DelegationStatus) -> &'static str {
    match s {
        DelegationStatus::Planned => "planned",
        DelegationStatus::Running => "running",
        DelegationStatus::Completed => "completed",
    }
}
