//! Read-only Kanban board queries exposed to launched agents via the MCP
//! kanban server: list/filter tasks, board overview, and bulk note reads.
//!
//! Every query is anchored on the calling agent's own task id so it stays
//! scoped to that task's board (the agent never sees other projects' boards).

use super::super::types::*;
use super::KanbanError;

impl super::WebStateHandle {
    /// Resolve the board a task belongs to, returning `(task, board)`.
    async fn board_for_task(&self, id: &str) -> Result<(Task, Board), KanbanError> {
        let task = self.db.kanban_task(id).ok_or(KanbanError::NotFound)?;
        let board = self
            .db
            .kanban_board(&task.board_id)
            .ok_or(KanbanError::NotFound)?;
        Ok((task, board))
    }

    /// List tasks on the anchor task's board, optionally filtered by lane
    /// (id or name), tags (match-any, case-insensitive), and a free-text query
    /// matched against title / description / tags. Archived tasks are excluded
    /// unless `include_archived` is set. Results are board-scoped.
    pub async fn kanban_query_tasks(
        &self,
        anchor_id: &str,
        lane: Option<&str>,
        tags: &[String],
        text: Option<&str>,
        include_archived: bool,
    ) -> Result<(Board, Vec<Task>), KanbanError> {
        let (_, board) = self.board_for_task(anchor_id).await?;

        // Resolve an optional lane filter (id or display name) to a lane id.
        let lane_id = match lane {
            Some(l) if !l.is_empty() => Some(
                board
                    .lanes
                    .iter()
                    .find(|x| x.id == l || x.name.eq_ignore_ascii_case(l))
                    .ok_or_else(|| KanbanError::Forbidden(format!("Unknown lane '{l}'")))?
                    .id
                    .clone(),
            ),
            _ => None,
        };
        let text_lc = text.filter(|t| !t.is_empty()).map(|t| t.to_lowercase());
        let tags_lc: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

        let tasks = self
            .db
            .kanban_tasks_for_board(&board.id)
            .into_iter()
            .filter(|t| include_archived || !t.archived)
            .filter(|t| lane_id.as_ref().is_none_or(|lid| &t.lane_id == lid))
            .filter(|t| {
                tags_lc.is_empty()
                    || t.tags
                        .iter()
                        .any(|tag| tags_lc.contains(&tag.to_lowercase()))
            })
            .filter(|t| match &text_lc {
                None => true,
                Some(q) => {
                    t.title.to_lowercase().contains(q)
                        || t.description.to_lowercase().contains(q)
                        || t.tags.iter().any(|tag| tag.to_lowercase().contains(q))
                }
            })
            .collect();
        Ok((board, tasks))
    }

    /// Board overview: the board plus all its tasks (used to compute per-lane
    /// counts and surface the lane graph to the agent).
    pub async fn kanban_board_overview(
        &self,
        anchor_id: &str,
    ) -> Result<(Board, Vec<Task>), KanbanError> {
        let (_, board) = self.board_for_task(anchor_id).await?;
        let tasks = self.db.kanban_tasks_for_board(&board.id);
        Ok((board, tasks))
    }

    /// Read the activity notes for one or more tasks on the anchor's board.
    /// Ids not on the same board are silently skipped (board isolation). An
    /// empty `task_ids` defaults to the anchor task itself.
    pub async fn kanban_read_notes(
        &self,
        anchor_id: &str,
        task_ids: &[String],
    ) -> Result<Vec<(Task, Vec<KanbanNote>)>, KanbanError> {
        let (anchor, board) = self.board_for_task(anchor_id).await?;
        let ids: Vec<String> = if task_ids.is_empty() {
            vec![anchor.id.clone()]
        } else {
            task_ids.to_vec()
        };

        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(task) = self.db.kanban_task(&id) else {
                continue;
            };
            if task.board_id != board.id {
                continue;
            }
            let notes = self.db.kanban_notes_for_task(&id);
            out.push((task, notes));
        }
        Ok(out)
    }
}
