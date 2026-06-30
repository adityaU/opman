//! Loopback-only internal Kanban *query* API, called by the `opman mcp-kanban`
//! server. Read-only board introspection for launched agents: list/filter
//! tasks, board overview, and bulk note reads. Auth + error mapping mirror
//! [`super::kanban_internal`].

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use serde_json::json;

use serde::Deserialize;

use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::super::web_state::KanbanError;

/// Request to list/filter a board's tasks.
#[derive(Debug, Default, Deserialize)]
pub struct InternalQueryRequest {
    /// Restrict to a single lane (id or display name).
    #[serde(default)]
    pub lane: Option<String>,
    /// Match-any tag filter (case-insensitive).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Free-text query matched against title / description / tags.
    #[serde(default)]
    pub query: Option<String>,
    /// Include archived tasks (default false).
    #[serde(default)]
    pub include_archived: bool,
}

/// Request to read notes for multiple tasks.
#[derive(Debug, Default, Deserialize)]
pub struct InternalNotesRequest {
    /// Task ids to read notes for. Empty defaults to the anchor task.
    #[serde(default)]
    pub task_ids: Vec<String>,
}

fn check_internal_token(state: &ServerState, headers: &HeaderMap) -> WebResult<()> {
    let provided = headers
        .get("x-internal-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !state.internal_token.is_empty() && provided == state.internal_token {
        return Ok(());
    }
    Err(WebError::Unauthorized)
}

fn map_err(e: KanbanError) -> WebError {
    match e {
        KanbanError::NotFound => WebError::NotFound("task"),
        KanbanError::Forbidden(msg) => WebError::Upstream(StatusCode::CONFLICT, msg),
    }
}

/// Compact task summary for list/query responses (omits launch internals).
fn task_summary(task: &Task, board: &Board) -> serde_json::Value {
    json!({
        "id": task.id,
        "title": task.title,
        "description": task.description,
        "tags": task.tags,
        "priority": task.priority,
        "lane_id": task.lane_id,
        "lane": board.lane(&task.lane_id).map(|l| l.name.clone()),
        "run_state": task.run_state,
        "archived": task.archived,
        "updated_at": task.updated_at,
    })
}

/// POST /internal/kanban/task/{id}/query — list/filter tasks on the board the
/// anchor task `{id}` belongs to.
pub async fn internal_query_tasks(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InternalQueryRequest>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let (board, tasks) = state
        .web_state
        .kanban_query_tasks(
            &id,
            req.lane.as_deref(),
            &req.tags,
            req.query.as_deref(),
            req.include_archived,
        )
        .await
        .map_err(map_err)?;

    let items: Vec<_> = tasks.iter().map(|t| task_summary(t, &board)).collect();
    Ok(Json(json!({ "count": items.len(), "tasks": items })))
}

/// GET /internal/kanban/task/{id}/board — board overview: lanes with task counts.
pub async fn internal_board_overview(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let (board, tasks) = state
        .web_state
        .kanban_board_overview(&id)
        .await
        .map_err(map_err)?;

    let lanes: Vec<_> = board
        .lanes
        .iter()
        .map(|l| {
            let in_lane = tasks.iter().filter(|t| t.lane_id == l.id);
            let active = in_lane.clone().filter(|t| !t.archived).count();
            let archived = in_lane.filter(|t| t.archived).count();
            json!({
                "id": l.id,
                "name": l.name,
                "terminal": l.terminal,
                "wip": l.wip,
                "active_count": active,
                "archived_count": archived,
                "next_lanes": board.transitions.get(&l.id).cloned().unwrap_or_default(),
            })
        })
        .collect();

    Ok(Json(json!({
        "board_id": board.id,
        "board_name": board.name,
        "total_active": tasks.iter().filter(|t| !t.archived).count(),
        "lanes": lanes,
    })))
}

/// POST /internal/kanban/task/{id}/notes — read notes for multiple tasks on the
/// anchor task `{id}`'s board.
pub async fn internal_read_notes(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InternalNotesRequest>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let results = state
        .web_state
        .kanban_read_notes(&id, &req.task_ids)
        .await
        .map_err(map_err)?;

    let tasks: Vec<_> = results
        .into_iter()
        .map(|(task, notes)| {
            let notes: Vec<_> = notes
                .iter()
                .map(|n| {
                    json!({
                        "author": n.author,
                        "body": n.body,
                        "lane_from": n.lane_from,
                        "lane_to": n.lane_to,
                        "created_at": n.created_at,
                    })
                })
                .collect();
            json!({
                "id": task.id,
                "title": task.title,
                "lane_id": task.lane_id,
                "note_count": notes.len(),
                "notes": notes,
            })
        })
        .collect();

    Ok(Json(json!({ "tasks": tasks })))
}
