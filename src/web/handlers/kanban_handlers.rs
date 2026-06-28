//! Kanban board HTTP handlers: board/lane config, task CRUD, attachments,
//! asset serving, and task launch (spawns an agent session).

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::super::web_state::KanbanError;

const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 200 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct BoardQuery {
    #[serde(default)]
    pub pi: Option<usize>,
}

fn map_kanban_err(e: KanbanError) -> WebError {
    match e {
        KanbanError::NotFound => WebError::NotFound("task"),
        KanbanError::Forbidden(msg) => WebError::Upstream(StatusCode::CONFLICT, msg),
    }
}

// ── Board ───────────────────────────────────────────────────────────

/// GET /api/kanban/board?pi={index}
pub async fn get_board(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(q): Query<BoardQuery>,
) -> WebResult<impl IntoResponse> {
    let resp = state
        .web_state
        .get_kanban_board(q.pi)
        .await
        .ok_or(WebError::BadRequest("No active project".into()))?;
    Ok(Json(resp))
}

/// PUT /api/kanban/board/{id}/config
pub async fn update_board_config(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(board_id): Path<String>,
    Json(req): Json<BoardConfigRequest>,
) -> WebResult<impl IntoResponse> {
    let board = state
        .web_state
        .update_kanban_board_config(&board_id, req.lanes, req.transitions)
        .await
        .ok_or(WebError::NotFound("board"))?;
    Ok(Json(json!({ "board": board })))
}

// ── Tasks ───────────────────────────────────────────────────────────

/// POST /api/kanban/task
pub async fn create_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<CreateTaskRequest>,
) -> WebResult<impl IntoResponse> {
    let task = state
        .web_state
        .create_kanban_task(req)
        .await
        .ok_or(WebError::NotFound("board"))?;
    Ok(Json(task))
}

/// PATCH /api/kanban/task/{id}
pub async fn update_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateTaskRequest>,
) -> WebResult<impl IntoResponse> {
    let task = state
        .web_state
        .update_kanban_task(&id, req)
        .await
        .map_err(map_kanban_err)?;
    Ok(Json(task))
}

/// GET /api/kanban/task/{id}
pub async fn get_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> WebResult<impl IntoResponse> {
    let detail = state
        .web_state
        .get_kanban_task_detail(&id)
        .await
        .ok_or(WebError::NotFound("task"))?;
    Ok(Json(detail))
}

/// DELETE /api/kanban/task/{id}
pub async fn delete_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.delete_kanban_task(&id).await {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err(WebError::NotFound("task"))
    }
}

// ── Attachments ─────────────────────────────────────────────────────

/// POST /api/kanban/task/{id}/attachment (multipart, field "file")
pub async fn upload_attachment(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> WebResult<impl IntoResponse> {
    // Confirm the task exists before accepting bytes.
    if state.web_state.kanban_get_task(&id).await.is_none() {
        return Err(WebError::NotFound("task"));
    }

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| WebError::BadRequest(format!("multipart error: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let raw_name = field.file_name().unwrap_or("upload").to_string();
        let mime = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| guess_mime(&raw_name));
        let data = field
            .bytes()
            .await
            .map_err(|e| WebError::BadRequest(format!("read error: {e}")))?;

        let cap = match kind_from_mime(&mime) {
            "image" => MAX_IMAGE_BYTES,
            "video" => MAX_VIDEO_BYTES,
            _ => MAX_FILE_BYTES,
        };
        if data.len() > cap {
            return Err(WebError::BadRequest(format!(
                "file too large ({} bytes, max {})",
                data.len(),
                cap
            )));
        }

        let safe = sanitize_filename(&raw_name);
        let dir = assets_dir(&id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| WebError::Internal(format!("mkdir failed: {e}")))?;
        let target = dir.join(&safe);
        tokio::fs::write(&target, &data)
            .await
            .map_err(|e| WebError::Internal(format!("write failed: {e}")))?;

        let attachment = state
            .web_state
            .add_kanban_attachment(&id, &safe, &mime, data.len() as i64)
            .await
            .ok_or(WebError::NotFound("task"))?;
        return Ok(Json(attachment));
    }
    Err(WebError::BadRequest("no 'file' field in upload".into()))
}

/// GET /api/kanban/asset/{task_id}/{filename}
pub async fn serve_asset(
    State(_state): State<ServerState>,
    _auth: AuthUser,
    Path((task_id, filename)): Path<(String, String)>,
    headers: HeaderMap,
) -> WebResult<Response> {
    let safe = sanitize_filename(&filename);
    let dir = assets_dir(&task_id);
    let target = dir.join(&safe);

    // Confine to the asset dir (defence in depth on top of sanitisation).
    let canon_dir = dir
        .canonicalize()
        .map_err(|_| WebError::NotFound("asset"))?;
    let canon_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("asset"))?;
    if !canon_target.starts_with(&canon_dir) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let bytes = tokio::fs::read(&canon_target)
        .await
        .map_err(|_| WebError::NotFound("asset"))?;
    let mime = guess_mime(&safe);
    let total = bytes.len();

    // Minimal single-range support so <video> seeking works.
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, total) {
            let slice = bytes[start..=end].to_vec();
            return Ok(Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, mime)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{total}"),
                )
                .header(header::CONTENT_LENGTH, slice.len())
                .body(Body::from(slice))
                .unwrap());
        }
    }

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, total)
        .body(Body::from(bytes))
        .unwrap())
}

// ── Launch / abort ──────────────────────────────────────────────────

/// POST /api/kanban/task/{id}/launch
pub async fn launch_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<LaunchTaskRequest>,
) -> WebResult<impl IntoResponse> {
    let task = state
        .web_state
        .kanban_get_task(&id)
        .await
        .ok_or(WebError::NotFound("task"))?;
    if task.run_state == "running" || task.run_state == "launching" {
        return Err(WebError::BadRequest("task already running".into()));
    }
    let board = state
        .web_state
        .kanban_get_board(&task.board_id)
        .await
        .ok_or(WebError::NotFound("board"))?;
    let lane = board.lane(&task.lane_id);
    let agent = req.agent.or_else(|| lane.and_then(|l| l.agent.clone()));
    let model = req.model.or_else(|| lane.and_then(|l| l.model.clone()));
    let project_path = board.project_path.clone();

    let base = crate::app::base_url().to_string();
    let client = &state.http_client;

    // 1) Create the session in the active backend.
    let create = client
        .post(format!("{base}/session"))
        .header("x-opencode-directory", &project_path)
        .json(&json!({ "title": task.title }))
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("create session failed: {e}")))?;
    let created: Value = create
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("bad create response: {e}")))?;
    let session_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(WebError::Internal("session id missing".into()))?
        .to_string();

    // 2) Seed the brief and dispatch the first turn.
    let brief = build_brief(&task, &board);
    let mut body = json!({ "parts": [{ "type": "text", "text": brief }] });
    if let Some(m) = &model {
        body["model"] = json!({ "providerID": "anthropic", "modelID": m });
    }
    if let Some(a) = &agent {
        body["agent"] = json!(a);
    }
    client
        .post(format!("{base}/session/{session_id}/message"))
        .header("x-opencode-directory", &project_path)
        .json(&body)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("dispatch failed: {e}")))?;

    // 3) Record launch metadata + broadcast.
    state
        .web_state
        .set_kanban_task_launch(&id, Some(session_id.clone()), model, agent, "running")
        .await;

    Ok(Json(json!({ "session_id": session_id })))
}

/// POST /api/kanban/task/{id}/abort
pub async fn abort_task(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> WebResult<impl IntoResponse> {
    let task = state
        .web_state
        .kanban_get_task(&id)
        .await
        .ok_or(WebError::NotFound("task"))?;
    if let Some(sid) = &task.session_id {
        let base = crate::app::base_url().to_string();
        let _ = state
            .http_client
            .post(format!("{base}/session/{sid}/abort"))
            .send()
            .await;
    }
    state
        .web_state
        .set_kanban_task_launch(
            &id,
            task.session_id.clone(),
            task.launch_model.clone(),
            task.launch_agent.clone(),
            "idle",
        )
        .await;
    Ok(Json(json!({ "ok": true })))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn build_brief(task: &Task, board: &Board) -> String {
    let lanes: Vec<String> = board.lanes.iter().map(|l| l.name.clone()).collect();
    let current = board
        .lane(&task.lane_id)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| task.lane_id.clone());
    let allowed: Vec<String> = board
        .transitions
        .get(&task.lane_id)
        .map(|ids| {
            ids.iter()
                .filter_map(|i| board.lane(i).map(|l| l.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let terminal = board
        .terminal_lane_id()
        .and_then(|tid| board.lane(tid))
        .map(|l| l.name.clone())
        .unwrap_or_else(|| "In Review".to_string());
    let tags = if task.tags.is_empty() {
        "(none)".to_string()
    } else {
        task.tags.join(", ")
    };

    format!(
        "You are working on a Kanban task. Use the `kanban` MCP tools to read details and report \
progress. Pass task_id=\"{task_id}\" to every kanban tool call.\n\n\
TASK: {title}\nTAGS: {tags}\nPRIORITY: {priority}\nCURRENT LANE: {current}\n\
ALL LANES: {lanes}\nYOU MAY MOVE TO: {allowed}\n\n\
BRIEF:\n{desc}\n\n\
Workflow:\n\
1. Call kanban_get_task(task_id) to confirm details and see the lanes you can move to.\n\
2. As you progress, call kanban_set_lane(task_id, lane) to reflect your current stage \
(only along allowed transitions).\n\
3. Call kanban_add_note(task_id, body) at meaningful milestones so the board shows live progress.\n\
4. When the work is ready for human review, call kanban_complete(task_id, summary) — this moves \
the task to \"{terminal}\" for human sign-off. Do not move it past that.\n\
Begin now.",
        task_id = task.id,
        title = task.title,
        priority = task.priority,
        lanes = lanes.join(" → "),
        allowed = if allowed.is_empty() { "(none)".to_string() } else { allowed.join(", ") },
        desc = task.description,
    )
}

/// Directory holding a task's uploaded assets (outside any project repo).
fn assets_dir(task_id: &str) -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("opman")
        .join("kanban")
        .join("assets")
        .join(sanitize_filename(task_id))
}

/// Strip path separators / traversal and leading dots from a name.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .replace("..", "_")
        .replace(['/', '\\'], "_")
        .trim_start_matches('.')
        .to_string();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned
    }
}

/// Parse a single `bytes=start-end` range. Returns inclusive (start, end).
fn parse_range(range: &str, total: usize) -> Option<(usize, usize)> {
    if total == 0 {
        return None;
    }
    let spec = range.strip_prefix("bytes=")?;
    let (s, e) = spec.split_once('-')?;
    let start: usize = s.trim().parse().ok()?;
    let end: usize = if e.trim().is_empty() {
        total - 1
    } else {
        e.trim().parse::<usize>().ok()?.min(total - 1)
    };
    if start > end {
        return None;
    }
    Some((start, end))
}

fn guess_mime(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let m = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        _ => "application/octet-stream",
    };
    m.to_string()
}
