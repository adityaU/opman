//! Kanban board state operations: thin wrappers over the SQLite CRUD that also
//! resolve the per-project board and broadcast SSE events on every change.

use chrono::Utc;

use super::super::types::*;
use super::uuid_like_id;

/// Failure modes for task mutations.
pub enum KanbanError {
    NotFound,
    /// Illegal lane transition (not an edge in the board's graph).
    Forbidden(String),
}

impl super::WebStateHandle {
    /// Resolve a project path by index (or the active project). Canonicalized
    /// best-effort so the board key is stable across reindexing.
    async fn kanban_project_path(&self, pi: Option<usize>) -> Option<String> {
        let state = self.inner.read().await;
        let idx = pi.unwrap_or(state.active_project);
        let path = state.projects.get(idx)?.path.clone();
        let resolved = std::fs::canonicalize(&path).unwrap_or(path);
        Some(resolved.to_string_lossy().to_string())
    }

    /// Get (or lazily create) the board for a project + all its tasks.
    pub async fn get_kanban_board(&self, pi: Option<usize>) -> Option<BoardResponse> {
        let project_path = self.kanban_project_path(pi).await?;
        let board = match self.db.kanban_board_for_project(&project_path) {
            Some(b) => b,
            None => {
                let now = Utc::now().to_rfc3339();
                let b = default_board(format!("brd_{}", uuid_like_id()), project_path.clone());
                self.db.insert_kanban_board(&b, &now);
                b
            }
        };
        let tasks = self.db.kanban_tasks_for_board(&board.id);
        Some(BoardResponse { board, tasks })
    }

    /// Replace a board's lanes + transition graph.
    pub async fn update_kanban_board_config(
        &self,
        board_id: &str,
        lanes: Vec<Lane>,
        transitions: Transitions,
    ) -> Option<Board> {
        let mut board = self.db.kanban_board(board_id)?;
        board.lanes = lanes;
        board.transitions = transitions;
        let now = Utc::now().to_rfc3339();
        self.db.update_kanban_board_config(&board, &now);
        self.broadcast_board(&board.project_path);
        Some(board)
    }

    /// Create a task, appended to the end of its lane.
    pub async fn create_kanban_task(&self, req: CreateTaskRequest) -> Option<Task> {
        let board = self.db.kanban_board(&req.board_id)?;
        let now = Utc::now().to_rfc3339();
        let order_index = self.db.kanban_max_order(&board.id, &req.lane_id) + 1.0;
        let task = Task {
            id: format!("tsk_{}", uuid_like_id()),
            board_id: req.board_id,
            lane_id: req.lane_id,
            title: req.title,
            description: req.description,
            tags: req.tags,
            priority: req.priority,
            order_index,
            session_id: None,
            launch_model: None,
            launch_agent: None,
            run_state: "idle".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.db.insert_kanban_task(&task);
        self.broadcast_task(&board.project_path, &task.id);
        Some(task)
    }

    /// Edit a task and/or move it. Lane moves are validated against the graph.
    pub async fn update_kanban_task(
        &self,
        id: &str,
        req: UpdateTaskRequest,
    ) -> Result<Task, KanbanError> {
        let mut task = self.db.kanban_task(id).ok_or(KanbanError::NotFound)?;
        let board = self.db.kanban_board(&task.board_id).ok_or(KanbanError::NotFound)?;

        if let Some(ref new_lane) = req.lane_id {
            if new_lane != &task.lane_id && !board.transition_allowed(&task.lane_id, new_lane) {
                return Err(KanbanError::Forbidden(format!(
                    "Transition {} → {} is not allowed",
                    task.lane_id, new_lane
                )));
            }
            task.lane_id = new_lane.clone();
        }
        if let Some(v) = req.title {
            task.title = v;
        }
        if let Some(v) = req.description {
            task.description = v;
        }
        if let Some(v) = req.tags {
            task.tags = v;
        }
        if let Some(v) = req.priority {
            task.priority = v;
        }
        if let Some(v) = req.order_index {
            task.order_index = v;
        }
        task.updated_at = Utc::now().to_rfc3339();
        self.db.update_kanban_task(&task);
        self.broadcast_task(&board.project_path, &task.id);
        Ok(task)
    }

    pub async fn delete_kanban_task(&self, id: &str) -> bool {
        let Some(task) = self.db.kanban_task(id) else {
            return false;
        };
        let project_path = self
            .db
            .kanban_board(&task.board_id)
            .map(|b| b.project_path)
            .unwrap_or_default();
        let ok = self.db.delete_kanban_task(id);
        if ok {
            self.broadcast_task(&project_path, id);
        }
        ok
    }

    /// Full task detail (notes + attachments with public URLs).
    pub async fn get_kanban_task_detail(&self, id: &str) -> Option<TaskDetail> {
        let task = self.db.kanban_task(id)?;
        let notes = self.db.kanban_notes_for_task(id);
        let mut attachments = self.db.kanban_attachments_for_task(id);
        for a in &mut attachments {
            a.url = asset_url(id, &a.filename);
        }
        Some(TaskDetail {
            task,
            notes,
            attachments,
        })
    }

    /// Record an uploaded attachment (the binary is written to disk by the handler).
    pub async fn add_kanban_attachment(
        &self,
        task_id: &str,
        filename: &str,
        mime: &str,
        size_bytes: i64,
    ) -> Option<Attachment> {
        let task = self.db.kanban_task(task_id)?;
        let now = Utc::now().to_rfc3339();
        let attachment = Attachment {
            id: format!("att_{}", uuid_like_id()),
            task_id: task_id.to_string(),
            filename: filename.to_string(),
            mime: mime.to_string(),
            kind: kind_from_mime(mime).to_string(),
            size_bytes,
            created_at: now,
            url: asset_url(task_id, filename),
        };
        self.db.insert_kanban_attachment(&attachment);
        if let Some(board) = self.db.kanban_board(&task.board_id) {
            self.broadcast_task(&board.project_path, task_id);
        }
        Some(attachment)
    }

    /// Record launch metadata on a task (called by the launch handler).
    pub async fn set_kanban_task_launch(
        &self,
        id: &str,
        session_id: Option<String>,
        model: Option<String>,
        agent: Option<String>,
        run_state: &str,
    ) -> Option<Task> {
        let mut task = self.db.kanban_task(id)?;
        task.session_id = session_id;
        task.launch_model = model;
        task.launch_agent = agent;
        task.run_state = run_state.to_string();
        task.updated_at = Utc::now().to_rfc3339();
        self.db.update_kanban_task(&task);
        if let Some(board) = self.db.kanban_board(&task.board_id) {
            self.broadcast_task(&board.project_path, id);
        }
        Some(task)
    }

    /// Fetch a single task (no notes/attachments).
    pub async fn kanban_get_task(&self, id: &str) -> Option<Task> {
        self.db.kanban_task(id)
    }

    /// Fetch a board by id.
    pub async fn kanban_get_board(&self, board_id: &str) -> Option<Board> {
        self.db.kanban_board(board_id)
    }

    // ── Internal (MCP-facing) ───────────────────────────────────────

    /// Move a task to `lane` (by id or name), enforcing the transition graph,
    /// and record a note describing the move.
    pub async fn kanban_internal_set_lane(
        &self,
        id: &str,
        lane: &str,
        run_state: Option<String>,
    ) -> Result<Task, KanbanError> {
        let mut task = self.db.kanban_task(id).ok_or(KanbanError::NotFound)?;
        let board = self.db.kanban_board(&task.board_id).ok_or(KanbanError::NotFound)?;
        // Accept a lane id or a (case-insensitive) lane name.
        let target = board
            .lanes
            .iter()
            .find(|l| l.id == lane || l.name.eq_ignore_ascii_case(lane))
            .ok_or_else(|| KanbanError::Forbidden(format!("Unknown lane '{lane}'")))?
            .id
            .clone();
        let from = task.lane_id.clone();
        if target != from && !board.transition_allowed(&from, &target) {
            return Err(KanbanError::Forbidden(format!(
                "Transition {from} → {target} is not allowed"
            )));
        }
        let now = Utc::now().to_rfc3339();
        task.lane_id = target.clone();
        if let Some(rs) = run_state {
            task.run_state = rs;
        }
        task.order_index = self.db.kanban_max_order(&board.id, &target) + 1.0;
        task.updated_at = now.clone();
        self.db.update_kanban_task(&task);
        if from != target {
            let note = KanbanNote {
                id: format!("nte_{}", uuid_like_id()),
                author: "agent".to_string(),
                body: format!("Moved to {target}"),
                lane_from: Some(from),
                lane_to: Some(target),
                created_at: now,
            };
            self.db.insert_kanban_note(&note, id);
        }
        self.broadcast_task(&board.project_path, id);
        Ok(task)
    }

    /// Complete a task: move to the board's terminal review lane (bypassing the
    /// normal graph, since completion is an explicit terminal action), mark it
    /// `done`, and record a summary note.
    pub async fn kanban_internal_complete(
        &self,
        id: &str,
        summary: &str,
    ) -> Result<Task, KanbanError> {
        let mut task = self.db.kanban_task(id).ok_or(KanbanError::NotFound)?;
        let board = self.db.kanban_board(&task.board_id).ok_or(KanbanError::NotFound)?;
        let from = task.lane_id.clone();
        let target = board
            .terminal_lane_id()
            .map(|s| s.to_string())
            .unwrap_or(from.clone());
        let now = Utc::now().to_rfc3339();
        task.lane_id = target.clone();
        task.run_state = "done".to_string();
        task.order_index = self.db.kanban_max_order(&board.id, &target) + 1.0;
        task.updated_at = now.clone();
        self.db.update_kanban_task(&task);
        let note = KanbanNote {
            id: format!("nte_{}", uuid_like_id()),
            author: "agent".to_string(),
            body: if summary.is_empty() {
                "Completed — ready for review".to_string()
            } else {
                summary.to_string()
            },
            lane_from: Some(from),
            lane_to: Some(target),
            created_at: now,
        };
        self.db.insert_kanban_note(&note, id);
        self.broadcast_task(&board.project_path, id);
        Ok(task)
    }

    /// Append a progress note to a task's timeline.
    pub async fn kanban_internal_note(
        &self,
        id: &str,
        body: &str,
        lane_from: Option<String>,
        lane_to: Option<String>,
    ) -> Result<(), KanbanError> {
        let task = self.db.kanban_task(id).ok_or(KanbanError::NotFound)?;
        let note = KanbanNote {
            id: format!("nte_{}", uuid_like_id()),
            author: "agent".to_string(),
            body: body.to_string(),
            lane_from,
            lane_to,
            created_at: Utc::now().to_rfc3339(),
        };
        self.db.insert_kanban_note(&note, id);
        if let Some(board) = self.db.kanban_board(&task.board_id) {
            self.broadcast_task(&board.project_path, id);
        }
        Ok(())
    }

    // ── Broadcast helpers ───────────────────────────────────────────

    fn broadcast_task(&self, project_path: &str, task_id: &str) {
        let _ = self.event_tx.send(WebEvent::KanbanTaskUpdated {
            project_path: project_path.to_string(),
            task_id: task_id.to_string(),
        });
    }

    fn broadcast_board(&self, project_path: &str) {
        let _ = self.event_tx.send(WebEvent::KanbanBoardUpdated {
            project_path: project_path.to_string(),
        });
    }
}

fn asset_url(task_id: &str, filename: &str) -> String {
    format!("/api/kanban/asset/{task_id}/{filename}")
}
