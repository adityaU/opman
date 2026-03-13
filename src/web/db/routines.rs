//! Routine and routine-run CRUD backed by SQLite.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

// ── Routines ────────────────────────────────────────────────────────

impl Db {
    /// List all routines, sorted by updated_at DESC.
    pub fn list_routines(&self) -> Vec<RoutineDefinition> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, trigger, action, enabled, cron_expr, timezone,
                        target_mode, session_id, project_index, prompt,
                        provider_id, model_id, mission_id,
                        last_run_at, next_run_at, last_error,
                        created_at, updated_at
                 FROM routines ORDER BY updated_at DESC",
            )
            .expect("prepare list_routines");

        stmt.query_map([], |row| {
            Ok(RoutineDefinition {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger: parse_trigger(&row.get::<_, String>(2)?),
                action: parse_action(&row.get::<_, String>(3)?),
                enabled: row.get::<_, i64>(4)? != 0,
                cron_expr: row.get(5)?,
                timezone: row.get(6)?,
                target_mode: row
                    .get::<_, Option<String>>(7)?
                    .map(|s| parse_target_mode(&s)),
                session_id: row.get(8)?,
                project_index: row.get::<_, Option<i64>>(9)?.map(|v| v as usize),
                prompt: row.get(10)?,
                provider_id: row.get(11)?,
                model_id: row.get(12)?,
                mission_id: row.get(13)?,
                last_run_at: row.get(14)?,
                next_run_at: row.get(15)?,
                last_error: row.get(16)?,
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
            })
        })
        .expect("query list_routines")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a routine.
    pub fn insert_routine(&self, r: &RoutineDefinition) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO routines
                (id, name, trigger, action, enabled, cron_expr, timezone,
                 target_mode, session_id, project_index, prompt,
                 provider_id, model_id, mission_id,
                 last_run_at, next_run_at, last_error,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                r.id,
                r.name,
                trigger_str(&r.trigger),
                action_str(&r.action),
                r.enabled as i64,
                r.cron_expr,
                r.timezone,
                r.target_mode.as_ref().map(target_mode_str),
                r.session_id,
                r.project_index.map(|v| v as i64),
                r.prompt,
                r.provider_id,
                r.model_id,
                r.mission_id,
                r.last_run_at,
                r.next_run_at,
                r.last_error,
                r.created_at,
                r.updated_at,
            ],
        )
        .expect("insert_routine");
    }

    /// Update a routine by ID. Returns true if the row existed.
    #[allow(dead_code)]
    pub fn update_routine_row(&self, r: &RoutineDefinition) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute(
                "UPDATE routines SET name=?1, trigger=?2, action=?3,
                    enabled=?4, cron_expr=?5, timezone=?6,
                    target_mode=?7, session_id=?8, project_index=?9,
                    prompt=?10, provider_id=?11, model_id=?12, mission_id=?13,
                    last_run_at=?14, next_run_at=?15, last_error=?16,
                    updated_at=?17
                 WHERE id=?18",
                params![
                    r.name,
                    trigger_str(&r.trigger),
                    action_str(&r.action),
                    r.enabled as i64,
                    r.cron_expr,
                    r.timezone,
                    r.target_mode.as_ref().map(target_mode_str),
                    r.session_id,
                    r.project_index.map(|v| v as i64),
                    r.prompt,
                    r.provider_id,
                    r.model_id,
                    r.mission_id,
                    r.last_run_at,
                    r.next_run_at,
                    r.last_error,
                    r.updated_at,
                    r.id,
                ],
            )
            .expect("update_routine");
        changed > 0
    }

    /// Delete a routine by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_routine_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM routines WHERE id=?1", params![id])
            .expect("delete_routine");
        changed > 0
    }
}

// ── Routine Runs ────────────────────────────────────────────────────

impl Db {
    /// List all routine runs, sorted by created_at DESC.
    pub fn list_routine_runs(&self) -> Vec<RoutineRunRecord> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, routine_id, status, summary, target_session_id,
                        duration_ms, created_at
                 FROM routine_runs ORDER BY created_at DESC",
            )
            .expect("prepare list_routine_runs");

        stmt.query_map([], |row| {
            Ok(RoutineRunRecord {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                status: row.get(2)?,
                summary: row.get(3)?,
                target_session_id: row.get(4)?,
                duration_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                created_at: row.get(6)?,
            })
        })
        .expect("query list_routine_runs")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a routine run record.
    pub fn insert_routine_run(&self, r: &RoutineRunRecord) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO routine_runs (id, routine_id, status, summary,
             target_session_id, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                r.id,
                r.routine_id,
                r.status,
                r.summary,
                r.target_session_id,
                r.duration_ms.map(|v| v as i64),
                r.created_at
            ],
        )
        .expect("insert_routine_run");
    }
}

// ── String conversions ──────────────────────────────────────────────

pub(crate) fn trigger_str(t: &RoutineTrigger) -> &'static str {
    match t {
        RoutineTrigger::Manual => "manual",
        RoutineTrigger::Scheduled => "scheduled",
        RoutineTrigger::OnSessionIdle => "on_session_idle",
        RoutineTrigger::DailySummary => "daily_summary",
    }
}

pub(crate) fn parse_trigger(s: &str) -> RoutineTrigger {
    match s {
        "scheduled" => RoutineTrigger::Scheduled,
        "on_session_idle" => RoutineTrigger::OnSessionIdle,
        "daily_summary" => RoutineTrigger::DailySummary,
        _ => RoutineTrigger::Manual,
    }
}

pub(crate) fn action_str(a: &RoutineAction) -> &'static str {
    match a {
        RoutineAction::SendMessage => "send_message",
        RoutineAction::ReviewMission => "review_mission",
        RoutineAction::OpenInbox => "open_inbox",
        RoutineAction::OpenActivityFeed => "open_activity_feed",
    }
}

pub(crate) fn parse_action(s: &str) -> RoutineAction {
    match s {
        "send_message" => RoutineAction::SendMessage,
        "open_inbox" => RoutineAction::OpenInbox,
        "open_activity_feed" => RoutineAction::OpenActivityFeed,
        _ => RoutineAction::ReviewMission,
    }
}

pub(crate) fn target_mode_str(t: &RoutineTargetMode) -> &'static str {
    match t {
        RoutineTargetMode::ExistingSession => "existing_session",
        RoutineTargetMode::NewSession => "new_session",
    }
}

pub(crate) fn parse_target_mode(s: &str) -> RoutineTargetMode {
    match s {
        "new_session" => RoutineTargetMode::NewSession,
        _ => RoutineTargetMode::ExistingSession,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routine_round_trip() {
        let db = Db::open_memory().unwrap();
        let r = RoutineDefinition {
            id: "rt-1".into(),
            name: "Daily review".into(),
            trigger: RoutineTrigger::Scheduled,
            action: RoutineAction::SendMessage,
            enabled: true,
            cron_expr: Some("0 9 * * *".into()),
            timezone: Some("America/New_York".into()),
            target_mode: Some(RoutineTargetMode::ExistingSession),
            session_id: Some("sess-123".into()),
            project_index: Some(0),
            prompt: Some("Review the current state of the project".into()),
            provider_id: None,
            model_id: None,
            mission_id: None,
            last_run_at: None,
            next_run_at: None,
            last_error: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_routine(&r);
        let listed = db.list_routines();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Daily review");
        assert_eq!(listed[0].trigger, RoutineTrigger::Scheduled);
        assert_eq!(listed[0].action, RoutineAction::SendMessage);
        assert!(listed[0].enabled);
        assert_eq!(listed[0].cron_expr.as_deref(), Some("0 9 * * *"));
        assert_eq!(
            listed[0].prompt.as_deref(),
            Some("Review the current state of the project")
        );

        let run = RoutineRunRecord {
            id: "rr-1".into(),
            routine_id: "rt-1".into(),
            status: "completed".into(),
            summary: "All good".into(),
            target_session_id: Some("sess-123".into()),
            duration_ms: Some(1500),
            created_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_routine_run(&run);
        let runs = db.list_routine_runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].summary, "All good");
        assert_eq!(runs[0].target_session_id.as_deref(), Some("sess-123"));
        assert_eq!(runs[0].duration_ms, Some(1500));

        assert!(db.delete_routine_row("rt-1"));
        assert!(db.list_routines().is_empty());
    }
}
