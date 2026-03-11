//! Mission CRUD backed by SQLite (v2: goal-driven loop model).

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all missions, sorted by updated_at DESC.
    pub fn list_missions(&self) -> Vec<Mission> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, goal, session_id, project_index, state,
                        iteration, max_iterations, last_verdict,
                        last_eval_summary, eval_history,
                        created_at, updated_at
                 FROM missions ORDER BY updated_at DESC",
            )
            .expect("prepare list_missions");

        stmt.query_map([], |row| {
            let eval_history_json: String = row.get(9)?;
            let eval_history: Vec<EvalRecord> =
                serde_json::from_str(&eval_history_json).unwrap_or_default();
            let last_verdict_str: Option<String> = row.get(7)?;
            let last_verdict = last_verdict_str.and_then(|s| parse_eval_verdict(&s));

            Ok(Mission {
                id: row.get(0)?,
                goal: row.get(1)?,
                session_id: row.get(2)?,
                project_index: row.get::<_, i64>(3)? as usize,
                state: parse_mission_state(&row.get::<_, String>(4)?),
                iteration: row.get::<_, i64>(5)? as u32,
                max_iterations: row.get::<_, i64>(6)? as u32,
                last_verdict,
                last_eval_summary: row.get(8)?,
                eval_history,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .expect("query list_missions")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a new mission (used by tests; runtime uses db_sync snapshot).
    #[allow(dead_code)]
    pub fn insert_mission(&self, m: &Mission) {
        let conn = self.conn();
        let eval_history_json =
            serde_json::to_string(&m.eval_history).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO missions
                (id, goal, session_id, project_index, state,
                 iteration, max_iterations, last_verdict,
                 last_eval_summary, eval_history,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                m.id,
                m.goal,
                m.session_id,
                m.project_index as i64,
                mission_state_str(&m.state),
                m.iteration as i64,
                m.max_iterations as i64,
                m.last_verdict.as_ref().map(eval_verdict_str),
                m.last_eval_summary,
                eval_history_json,
                m.created_at,
                m.updated_at,
            ],
        )
        .expect("insert_mission");
    }

    /// Update a mission by ID. Returns true if the row existed.
    #[allow(dead_code)]
    pub fn update_mission_row(&self, m: &Mission) -> bool {
        let conn = self.conn();
        let eval_history_json =
            serde_json::to_string(&m.eval_history).unwrap_or_else(|_| "[]".to_string());
        let changed = conn
            .execute(
                "UPDATE missions SET goal=?1, session_id=?2, project_index=?3,
                    state=?4, iteration=?5, max_iterations=?6,
                    last_verdict=?7, last_eval_summary=?8,
                    eval_history=?9, updated_at=?10
                 WHERE id=?11",
                params![
                    m.goal,
                    m.session_id,
                    m.project_index as i64,
                    mission_state_str(&m.state),
                    m.iteration as i64,
                    m.max_iterations as i64,
                    m.last_verdict.as_ref().map(eval_verdict_str),
                    m.last_eval_summary,
                    eval_history_json,
                    m.updated_at,
                    m.id,
                ],
            )
            .expect("update_mission");
        changed > 0
    }

    /// Delete a mission by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_mission_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM missions WHERE id=?1", params![id])
            .expect("delete_mission");
        changed > 0
    }
}

fn mission_state_str(s: &MissionState) -> &'static str {
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

fn parse_mission_state(s: &str) -> MissionState {
    match s {
        "executing" => MissionState::Executing,
        "evaluating" => MissionState::Evaluating,
        "paused" => MissionState::Paused,
        "completed" => MissionState::Completed,
        "cancelled" => MissionState::Cancelled,
        "failed" => MissionState::Failed,
        // Legacy: map old statuses to pending
        "planned" | "active" | "blocked" | _ => MissionState::Pending,
    }
}

fn eval_verdict_str(v: &EvalVerdict) -> &'static str {
    match v {
        EvalVerdict::Achieved => "achieved",
        EvalVerdict::Continue => "continue",
        EvalVerdict::Blocked => "blocked",
        EvalVerdict::Failed => "failed",
    }
}

fn parse_eval_verdict(s: &str) -> Option<EvalVerdict> {
    match s {
        "achieved" => Some(EvalVerdict::Achieved),
        "continue" => Some(EvalVerdict::Continue),
        "blocked" => Some(EvalVerdict::Blocked),
        "failed" => Some(EvalVerdict::Failed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_round_trip() {
        let db = Db::open_memory().unwrap();
        let m = Mission {
            id: "m-1".into(),
            goal: "Ship v2 release".into(),
            session_id: "sess-1".into(),
            project_index: 0,
            state: MissionState::Executing,
            iteration: 2,
            max_iterations: 10,
            last_verdict: Some(EvalVerdict::Continue),
            last_eval_summary: Some("Tests passing, need docs".into()),
            eval_history: vec![EvalRecord {
                iteration: 1,
                verdict: EvalVerdict::Continue,
                summary: "Initial implementation done".into(),
                next_step: Some("Add tests".into()),
                timestamp: "2025-01-01T00:00:00Z".into(),
            }],
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_mission(&m);
        let list = db.list_missions();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].goal, "Ship v2 release");
        assert_eq!(list[0].iteration, 2);
        assert_eq!(list[0].eval_history.len(), 1);
        assert!(matches!(list[0].last_verdict, Some(EvalVerdict::Continue)));
        assert!(db.delete_mission_row("m-1"));
        assert!(db.list_missions().is_empty());
    }
}
