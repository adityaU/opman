//! Pipeline-mode kanban launches: each lane runs as its own agent session,
//! chained by feeding one stage's final output into the next stage's prompt.
//!
//! The single-session launch (one agent walks the whole board itself) lives in
//! the HTTP handler and is unchanged. This module drives the *staged* mode:
//! stages are computed from the board's lanes, the first session is started at
//! launch, and `try_advance_kanban_pipeline` (called from the idle hook) chains
//! the rest as each stage session finishes its turn.

use chrono::Utc;
use serde_json::json;

use super::super::types::*;
use super::kanban_pipeline_brief::{build_stage_brief, inject_memory, pipeline_stage_lanes, truncate};
use super::uuid_like_id;

impl super::WebStateHandle {
    /// Start a pipeline-mode launch. Computes the stages, starts stage 0, and
    /// returns its session id. Errors if no lane in the path has a stage config.
    pub async fn launch_kanban_pipeline(
        &self,
        task_id: &str,
        model: Option<String>,
        agent: Option<String>,
    ) -> Result<String, String> {
        let mut task = self.db.kanban_task(task_id).ok_or("task not found")?;
        let board = self.db.kanban_board(&task.board_id).ok_or("board not found")?;

        let stage_lanes = pipeline_stage_lanes(&board, &task.lane_id);
        if stage_lanes.is_empty() {
            return Err(
                "No pipeline stages: give at least one lane an agent or a prompt in Configure lanes."
                    .into(),
            );
        }

        let now = Utc::now().to_rfc3339();
        let mut run = PipelineRun {
            task_id: task_id.to_string(),
            stages: stage_lanes
                .iter()
                .map(|lid| PipelineStage {
                    lane_id: lid.clone(),
                    session_id: None,
                    status: "pending".to_string(),
                    output: None,
                })
                .collect(),
            current_index: 0,
            status: "running".to_string(),
            launch_model: model,
            launch_agent: agent,
            created_at: now.clone(),
            updated_at: now,
        };

        let session_id = self.start_stage(&mut task, &board, &run, 0, None).await?;
        run.stages[0].session_id = Some(session_id.clone());
        run.stages[0].status = "running".to_string();
        self.db.kanban_pipeline_upsert(&run);

        task.session_id = Some(session_id.clone());
        task.launch_model = run.launch_model.clone();
        task.launch_agent = run.launch_agent.clone();
        task.run_state = "running".to_string();
        task.updated_at = Utc::now().to_rfc3339();
        self.db.update_kanban_task(&task);
        self.notify_task(&board.project_path, task_id);
        Ok(session_id)
    }

    /// Stop a running pipeline (e.g. on abort) so a subsequent idle event from
    /// the aborted stage session does not chain the next stage. No-op if there is
    /// no running pipeline for the task.
    pub async fn stop_kanban_pipeline(&self, task_id: &str) {
        let Some(mut run) = self.db.kanban_pipeline_get(task_id) else {
            return;
        };
        if run.status != "running" {
            return;
        }
        run.status = "stopped".to_string();
        if let Some(stage) = run.stages.get_mut(run.current_index) {
            if stage.status == "running" {
                stage.status = "failed".to_string();
            }
        }
        run.updated_at = Utc::now().to_rfc3339();
        self.db.kanban_pipeline_upsert(&run);
    }

    /// Called from the SSE idle hook. If `session_id` is the current stage of a
    /// running pipeline, capture its output and chain the next stage (or finish).
    pub(super) async fn try_advance_kanban_pipeline(&self, session_id: &str) {
        let Some(mut run) = self.db.kanban_pipeline_by_session(session_id) else {
            return;
        };
        let Some(mut task) = self.db.kanban_task(&run.task_id) else {
            return;
        };
        let Some(board) = self.db.kanban_board(&task.board_id) else {
            return;
        };

        // Capture the finished stage's output, mark it done.
        let output = self.capture_session_output(&board.project_path, session_id).await;
        let idx = run.current_index;
        run.stages[idx].status = "done".to_string();
        run.stages[idx].output = output.clone();

        let next = idx + 1;
        if next < run.stages.len() {
            // Chain: start the next stage seeded with this stage's output.
            match self.start_stage(&mut task, &board, &run, next, output.clone()).await {
                Ok(sid) => {
                    run.current_index = next;
                    run.stages[next].session_id = Some(sid.clone());
                    run.stages[next].status = "running".to_string();
                    task.session_id = Some(sid);
                    task.run_state = "running".to_string();
                }
                Err(e) => {
                    run.status = "failed".to_string();
                    task.run_state = "failed".to_string();
                    self.record_note(&task.id, "agent", &format!("Pipeline stalled: {e}"));
                }
            }
        } else {
            // Final stage done: move to the terminal review lane and finish.
            run.status = "done".to_string();
            if let Some(term) = board.terminal_lane_id() {
                task.lane_id = term.to_string();
                task.order_index = self.db.kanban_max_order(&board.id, term) + 1.0;
            }
            task.run_state = "done".to_string();
            let summary = output
                .as_deref()
                .map(|s| truncate(s, 600))
                .unwrap_or_else(|| "Pipeline complete — ready for review".to_string());
            self.record_note(&task.id, "agent", &summary);
            let _ = self.event_tx.send(WebEvent::Toast {
                message: format!("Pipeline complete: {}", task.title),
                level: "success".to_string(),
            });
        }

        run.updated_at = Utc::now().to_rfc3339();
        self.db.kanban_pipeline_upsert(&run);
        task.updated_at = Utc::now().to_rfc3339();
        self.db.update_kanban_task(&task);
        self.notify_task(&board.project_path, &task.id);
    }

    /// Start one stage: move the task into the stage's lane, create a fresh
    /// session, and dispatch the stage prompt (with the prior output). Returns
    /// the new session id. Mutates `task.lane_id` to the stage lane.
    async fn start_stage(
        &self,
        task: &mut Task,
        board: &Board,
        run: &PipelineRun,
        index: usize,
        prev_output: Option<String>,
    ) -> Result<String, String> {
        let lane_id = run.stages[index].lane_id.clone();
        let lane = board.lane(&lane_id).ok_or("stage lane missing")?.clone();

        // Move the task into the stage's lane and record the transition.
        let from = task.lane_id.clone();
        task.lane_id = lane_id.clone();
        task.order_index = self.db.kanban_max_order(&board.id, &lane_id) + 1.0;
        if from != lane_id {
            let note = KanbanNote {
                id: format!("nte_{}", uuid_like_id()),
                author: "agent".to_string(),
                body: format!("Pipeline stage {}/{}: {}", index + 1, run.stages.len(), lane.name),
                lane_from: Some(from),
                lane_to: Some(lane_id.clone()),
                created_at: Utc::now().to_rfc3339(),
            };
            self.db.insert_kanban_note(&note, &task.id);
        }

        let model = lane.model.clone().or_else(|| run.launch_model.clone());
        let agent = lane.agent.clone().or_else(|| run.launch_agent.clone());
        let memory = self.kanban_active_memory(&board.project_path).await;
        let brief = build_stage_brief(task, &lane, index, run.stages.len(), prev_output.as_deref());
        let brief = inject_memory(&brief, &memory);

        let base = crate::app::base_url().to_string();
        let client = reqwest::Client::new();
        let dir = board.project_path.clone();

        let create = client
            .post(format!("{base}/session"))
            .header("x-opencode-directory", &dir)
            .json(&json!({ "title": format!("{} — {}", task.title, lane.name) }))
            .send()
            .await
            .map_err(|e| format!("create session failed: {e}"))?;
        let created: serde_json::Value = create
            .json()
            .await
            .map_err(|e| format!("bad create response: {e}"))?;
        let session_id = created
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("session id missing")?
            .to_string();

        let mut body = json!({ "parts": [{ "type": "text", "text": brief }] });
        if let Some(m) = &model {
            body["model"] = json!({ "providerID": "anthropic", "modelID": m });
        }
        if let Some(a) = &agent {
            body["agent"] = json!(a);
        }
        client
            .post(format!("{base}/session/{session_id}/message"))
            .header("x-opencode-directory", &dir)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("dispatch failed: {e}"))?;

        Ok(session_id)
    }

    /// Read a session's latest assistant message text (the stage's output).
    async fn capture_session_output(&self, dir: &str, session_id: &str) -> Option<String> {
        let base = crate::app::base_url().to_string();
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/session/{session_id}/message"))
            .header("x-opencode-directory", dir)
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        latest_assistant_output(&body)
    }

    fn record_note(&self, task_id: &str, author: &str, body: &str) {
        let note = KanbanNote {
            id: format!("nte_{}", uuid_like_id()),
            author: author.to_string(),
            body: body.to_string(),
            lane_from: None,
            lane_to: None,
            created_at: Utc::now().to_rfc3339(),
        };
        self.db.insert_kanban_note(&note, task_id);
    }

    fn notify_task(&self, project_path: &str, task_id: &str) {
        let _ = self.event_tx.send(WebEvent::KanbanTaskUpdated {
            project_path: project_path.to_string(),
            task_id: task_id.to_string(),
        });
    }
}

/// Pick the latest assistant message text from a `/session/{id}/message` payload
/// (array- or object-shaped). Returns `None` when there is no non-empty assistant
/// output. Extracted from `capture_session_output` so it is testable without a
/// live upstream session server.
fn latest_assistant_output(body: &serde_json::Value) -> Option<String> {
    let messages: Vec<serde_json::Value> = match body {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => o.values().cloned().collect(),
        _ => return None,
    };
    let latest = messages
        .iter()
        .filter(|m| m.pointer("/info/role").and_then(|v| v.as_str()) == Some("assistant"))
        .max_by_key(|m| m.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0))?;
    let text = super::assistant::extract_message_text(latest);
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
#[path = "kanban_pipeline_tests.rs"]
mod kanban_pipeline_tests;

#[cfg(test)]
#[path = "kanban_pipeline_upstream_tests.rs"]
mod kanban_pipeline_upstream_tests;

