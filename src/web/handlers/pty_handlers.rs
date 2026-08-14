//! Web PTY spawn/write/resize/rename/kill/list handlers.

use std::path::PathBuf;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::pty_manager::{PtyKind, PtyProgram, PtySession, SpawnSpec};
use super::super::types::*;

#[derive(Serialize)]
struct SpawnResponse {
    id: String,
    ok: bool,
}

/// Spawn a new web-owned PTY, or return the one already answering to that id.
pub async fn spawn_pty(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SpawnPtyRequest>,
) -> WebResult<impl IntoResponse> {
    let project = resolve_project(&state, req.project.as_deref()).await?;
    let program = resolve_program(&state, req.kind, req.session_id).await?;

    let spec = SpawnSpec {
        id: req.id.clone(),
        program,
        project,
        label: req.label,
        rows: req.rows.unwrap_or(24).clamp(1, 500),
        cols: req.cols.unwrap_or(80).clamp(1, 500),
    };

    state
        .pty_mgr
        .spawn(spec)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to spawn PTY: {e}")))?;

    Ok(Json(SpawnResponse {
        id: req.id,
        ok: true,
    }))
}

/// Where the PTY starts: the caller's project, else whichever one is active.
async fn resolve_project(state: &ServerState, requested: Option<&str>) -> WebResult<PathBuf> {
    if let Some(path) = requested.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    state
        .web_state
        .get_working_dir()
        .await
        .ok_or(WebError::BadRequest("No active project".into()))
}

/// Pair the requested kind with the arguments only that kind takes.
async fn resolve_program(
    state: &ServerState,
    kind: PtyKind,
    session_id: Option<String>,
) -> WebResult<PtyProgram> {
    match kind {
        PtyKind::Shell => Ok(PtyProgram::Shell),
        PtyKind::Neovim => Ok(PtyProgram::Neovim),
        PtyKind::Git => Ok(PtyProgram::Git),
        PtyKind::Opencode => {
            let session_id = match session_id {
                Some(sid) => Some(sid),
                None => state.web_state.active_session_id().await,
            };
            Ok(PtyProgram::Opencode { session_id })
        }
        PtyKind::ClaudeAttach => {
            // Resolve the opman session → its live claude background short id.
            let session_id = match session_id {
                Some(sid) => sid,
                None => state
                    .web_state
                    .active_session_id()
                    .await
                    .ok_or(WebError::BadRequest("No session to attach".into()))?,
            };
            let short_id = crate::claude_engine::short_id_for_session(&session_id).ok_or(
                WebError::BadRequest(
                    "This session has no running claude agent to attach to".into(),
                ),
            )?;
            Ok(PtyProgram::ClaudeAttach { short_id })
        }
    }
}

/// Write bytes to a web-owned PTY.
pub async fn pty_write(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyWriteRequest>,
) -> WebResult<impl IntoResponse> {
    let data = BASE64
        .decode(&req.data)
        .map_err(|e| WebError::BadRequest(format!("Invalid base64: {e}")))?;
    ok_or_missing(state.pty_mgr.write(&req.id, data).await)
}

/// Resize a web-owned PTY.
pub async fn pty_resize(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyResizeRequest>,
) -> WebResult<impl IntoResponse> {
    let rows = req.rows.clamp(1, 500);
    let cols = req.cols.clamp(1, 500);
    ok_or_missing(state.pty_mgr.resize(&req.id, rows, cols).await)
}

/// Rename a web-owned PTY as the shell picker shows it.
pub async fn pty_rename(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyRenameRequest>,
) -> WebResult<impl IntoResponse> {
    let label = req.label.trim();
    if label.is_empty() {
        return Err(WebError::BadRequest("Label cannot be empty".into()));
    }
    ok_or_missing(state.pty_mgr.rename(&req.id, label.to_owned()).await)
}

/// Kill a web-owned PTY. The only thing that ends a shell short of it exiting.
pub async fn pty_kill(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyKillRequest>,
) -> WebResult<impl IntoResponse> {
    ok_or_missing(state.pty_mgr.kill(&req.id).await)
}

/// Every live web PTY: its project, label, kind and whether it is busy.
///
/// One endpoint rather than the three it replaces. The picker needs the labels,
/// the window rail needs the activity and re-attaching needs to know the id is
/// still live — all of that is one walk of the same map, and splitting it meant
/// the picker could offer a shell the activity poll had already dropped.
pub async fn pty_sessions(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let live: Vec<PtySession> = state.pty_mgr.sessions().await;
    Ok(Json(live))
}

fn ok_or_missing(found: bool) -> WebResult<StatusCode> {
    if found {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

#[cfg(test)]
#[path = "pty_handlers_tests.rs"]
mod pty_handlers_tests;

#[cfg(test)]
#[path = "pty_handlers_live_tests.rs"]
mod pty_handlers_live_tests;

#[cfg(test)]
#[path = "pty_handlers_live_extra_tests.rs"]
mod pty_handlers_live_extra_tests;

#[cfg(test)]
#[path = "pty_handlers_attach_tests.rs"]
mod pty_handlers_attach_tests;
