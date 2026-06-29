//! Persistence for kanban pipeline runs (staged, multi-session launches).
//!
//! A run records the ordered stages of a pipeline launch — one per lane — and
//! which session each stage runs in, so the board can chain output between
//! stages and tag every stage session to its own lane.

use rusqlite::{params, OptionalExtension};

use super::Db;
use crate::web::types::*;

impl Db {
    /// Insert or replace a pipeline run for a task.
    pub fn kanban_pipeline_upsert(&self, run: &PipelineRun) {
        let conn = self.conn();
        let stages = serde_json::to_string(&run.stages).unwrap_or_else(|_| "[]".into());
        let _ = conn.execute(
            "INSERT INTO kanban_pipeline_runs
                (task_id, stages, current_index, status, launch_model, launch_agent, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(task_id) DO UPDATE SET
                stages=excluded.stages, current_index=excluded.current_index,
                status=excluded.status, launch_model=excluded.launch_model,
                launch_agent=excluded.launch_agent, updated_at=excluded.updated_at",
            params![
                run.task_id, stages, run.current_index as i64, run.status,
                run.launch_model, run.launch_agent, run.created_at, run.updated_at
            ],
        );
    }

    /// Fetch the pipeline run for a task, if any.
    pub fn kanban_pipeline_get(&self, task_id: &str) -> Option<PipelineRun> {
        let conn = self.conn();
        conn.query_row(
            "SELECT task_id, stages, current_index, status, launch_model, launch_agent,
                    created_at, updated_at
             FROM kanban_pipeline_runs WHERE task_id = ?1",
            params![task_id],
            row_to_run,
        )
        .optional()
        .ok()
        .flatten()
    }

    /// All pipeline runs whose task belongs to the given board.
    pub fn kanban_pipelines_for_board(&self, board_id: &str) -> Vec<PipelineRun> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT r.task_id, r.stages, r.current_index, r.status, r.launch_model,
                    r.launch_agent, r.created_at, r.updated_at
             FROM kanban_pipeline_runs r
             JOIN kanban_tasks t ON t.id = r.task_id
             WHERE t.board_id = ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![board_id], row_to_run)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Find the active run whose *current* stage runs in `session_id`. Used by the
    /// idle hook to advance the pipeline when a stage session finishes its turn.
    pub fn kanban_pipeline_by_session(&self, session_id: &str) -> Option<PipelineRun> {
        let conn = self.conn();
        let like = format!("%{session_id}%");
        let mut stmt = conn
            .prepare(
                "SELECT task_id, stages, current_index, status, launch_model, launch_agent,
                        created_at, updated_at
                 FROM kanban_pipeline_runs
                 WHERE status = 'running' AND stages LIKE ?1",
            )
            .ok()?;
        let runs: Vec<PipelineRun> = stmt
            .query_map(params![like], row_to_run)
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        // LIKE is a coarse filter; confirm the current stage really owns the session.
        runs.into_iter().find(|r| {
            r.stages
                .get(r.current_index)
                .and_then(|s| s.session_id.as_deref())
                == Some(session_id)
        })
    }
}

fn row_to_run(row: &rusqlite::Row) -> rusqlite::Result<PipelineRun> {
    let stages_json: String = row.get(1)?;
    let current: i64 = row.get(2)?;
    Ok(PipelineRun {
        task_id: row.get(0)?,
        stages: serde_json::from_str(&stages_json).unwrap_or_default(),
        current_index: current.max(0) as usize,
        status: row.get(3)?,
        launch_model: row.get(4)?,
        launch_agent: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
