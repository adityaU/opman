//! Loopback-only internal Kanban API, called by the `opman mcp-kanban` server.
//!
//! These routes are mounted OUTSIDE `/api` so they skip the `AuthUser` extractor;
//! instead each handler validates a shared `X-Internal-Token` secret that is
//! written to `~/.config/opman/internal.json` at startup.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use serde_json::json;

use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::super::web_state::KanbanError;

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

/// GET /internal/kanban/task/{id} — task brief + lanes the task may move to.
pub async fn internal_get_task(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let task = state
        .web_state
        .kanban_get_task(&id)
        .await
        .ok_or(WebError::NotFound("task"))?;
    let board = state
        .web_state
        .kanban_get_board(&task.board_id)
        .await
        .ok_or(WebError::NotFound("board"))?;

    let lane_obj = |l: &Lane| json!({ "id": l.id, "name": l.name, "terminal": l.terminal });
    let allowed: Vec<_> = board
        .transitions
        .get(&task.lane_id)
        .map(|ids| {
            ids.iter()
                .filter_map(|i| board.lane(i))
                .map(lane_obj)
                .collect()
        })
        .unwrap_or_default();
    let terminal = board
        .terminal_lane_id()
        .and_then(|tid| board.lane(tid))
        .map(lane_obj);

    // Expose uploaded attachments with their absolute on-disk path so an agent (via the
    // kanban MCP) can Read them directly — the HTTP asset URL is auth-gated and not
    // reachable from a launched agent.
    let attachments: Vec<_> = state
        .web_state
        .get_kanban_task_detail(&id)
        .await
        .map(|d| d.attachments)
        .unwrap_or_default()
        .into_iter()
        .map(|a| {
            let path = super::kanban_handlers::assets_dir(&task.id).join(&a.filename);
            json!({
                "filename": a.filename,
                "mime": a.mime,
                "kind": a.kind,
                "path": path.to_string_lossy(),
            })
        })
        .collect();

    Ok(Json(json!({
        "id": task.id,
        "title": task.title,
        "description": task.description,
        "tags": task.tags,
        "priority": task.priority,
        "current_lane": board.lane(&task.lane_id).map(lane_obj),
        "lanes": board.lanes.iter().map(lane_obj).collect::<Vec<_>>(),
        "allowed_transitions": allowed,
        "terminal_lane": terminal,
        "run_state": task.run_state,
        "attachments": attachments,
    })))
}

/// POST /internal/kanban/task/{id}/status — move lane (graph-enforced).
pub async fn internal_set_status(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InternalStatusRequest>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let task = state
        .web_state
        .kanban_internal_set_lane(&id, &req.lane, req.run_state)
        .await
        .map_err(map_err)?;
    Ok(Json(json!({ "ok": true, "lane_id": task.lane_id })))
}

/// POST /internal/kanban/task/{id}/note — append a progress note.
pub async fn internal_add_note(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InternalNoteRequest>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    state
        .web_state
        .kanban_internal_note(&id, &req.body, req.lane_from, req.lane_to)
        .await
        .map_err(map_err)?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /internal/kanban/task/{id}/complete — move to terminal review lane + done.
pub async fn internal_complete(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InternalNoteRequest>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;
    let task = state
        .web_state
        .kanban_internal_complete(&id, &req.body)
        .await
        .map_err(map_err)?;
    Ok(Json(json!({ "ok": true, "lane_id": task.lane_id })))
}
