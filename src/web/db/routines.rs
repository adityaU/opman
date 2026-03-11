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
                "SELECT id, name, trigger, action, mission_id,
                        session_id, created_at, updated_at
                 FROM routines ORDER BY updated_at DESC",
            )
            .expect("prepare list_routines");

        stmt.query_map([], |row| {
            Ok(RoutineDefinition {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger: parse_trigger(&row.get::<_, String>(2)?),
                action: parse_action(&row.get::<_, String>(3)?),
                mission_id: row.get(4)?,
                session_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
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
                (id, name, trigger, action, mission_id,
                 session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                r.id,
                r.name,
                trigger_str(&r.trigger),
                action_str(&r.action),
                r.mission_id,
                r.session_id,
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
                    mission_id=?4, session_id=?5, updated_at=?6
                 WHERE id=?7",
                params![
                    r.name,
                    trigger_str(&r.trigger),
                    action_str(&r.action),
                    r.mission_id,
                    r.session_id,
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
                "SELECT id, routine_id, status, summary, created_at
                 FROM routine_runs ORDER BY created_at DESC",
            )
            .expect("prepare list_routine_runs");

        stmt.query_map([], |row| {
            Ok(RoutineRunRecord {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                status: row.get(2)?,
                summary: row.get(3)?,
                created_at: row.get(4)?,
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
            "INSERT INTO routine_runs (id, routine_id, status, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![r.id, r.routine_id, r.status, r.summary, r.created_at],
        )
        .expect("insert_routine_run");
    }
}

// ── String conversions ──────────────────────────────────────────────

fn trigger_str(t: &RoutineTrigger) -> &'static str {
    match t {
        RoutineTrigger::Manual => "manual",
        RoutineTrigger::OnSessionIdle => "on_session_idle",
        RoutineTrigger::DailySummary => "daily_summary",
    }
}

fn parse_trigger(s: &str) -> RoutineTrigger {
    match s {
        "on_session_idle" => RoutineTrigger::OnSessionIdle,
        "daily_summary" => RoutineTrigger::DailySummary,
        _ => RoutineTrigger::Manual,
    }
}

fn action_str(a: &RoutineAction) -> &'static str {
    match a {
        RoutineAction::ReviewMission => "review_mission",
        RoutineAction::OpenInbox => "open_inbox",
        RoutineAction::OpenActivityFeed => "open_activity_feed",
    }
}

fn parse_action(s: &str) -> RoutineAction {
    match s {
        "open_inbox" => RoutineAction::OpenInbox,
        "open_activity_feed" => RoutineAction::OpenActivityFeed,
        _ => RoutineAction::ReviewMission,
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
            trigger: RoutineTrigger::DailySummary,
            action: RoutineAction::ReviewMission,
            mission_id: None,
            session_id: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_routine(&r);
        assert_eq!(db.list_routines().len(), 1);

        let run = RoutineRunRecord {
            id: "rr-1".into(),
            routine_id: "rt-1".into(),
            status: "completed".into(),
            summary: "All good".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_routine_run(&run);
        let runs = db.list_routine_runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].summary, "All good");

        assert!(db.delete_routine_row("rt-1"));
        assert!(db.list_routines().is_empty());
    }
}
