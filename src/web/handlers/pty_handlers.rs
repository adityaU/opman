//! Web PTY spawn/write/resize/kill/list handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;

#[derive(Serialize)]
struct SpawnResponse {
    id: String,
    ok: bool,
}

/// Spawn a new web-owned PTY (shell, neovim, gitui, or opencode).
pub async fn spawn_pty(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SpawnPtyRequest>,
) -> WebResult<impl IntoResponse> {
    let rows = req.rows.unwrap_or(24).clamp(1, 500);
    let cols = req.cols.unwrap_or(80).clamp(1, 500);

    // Get the working directory from the active project
    let working_dir = state
        .web_state
        .get_working_dir()
        .await
        .ok_or(WebError::BadRequest("No active project".into()))?;

    let result = match req.kind.as_str() {
        "shell" => {
            state
                .pty_mgr
                .spawn_shell(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "neovim" => {
            state
                .pty_mgr
                .spawn_neovim(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "git" => {
            state
                .pty_mgr
                .spawn_gitui(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "opencode" => {
            // Get the active session ID (if any) to attach to
            let session_id = req.session_id.clone().or_else(|| {
                // Try to get from web state synchronously — but we're in async context
                None
            });
            // We'll resolve session_id from web state if not provided
            let session_id = match session_id {
                Some(sid) => Some(sid),
                None => state.web_state.active_session_id().await,
            };
            state
                .pty_mgr
                .spawn_opencode(req.id.clone(), rows, cols, working_dir, session_id)
                .await
        }
        _ => {
            return Err(WebError::BadRequest(format!(
                "Unknown PTY kind: {}",
                req.kind
            )))
        }
    };

    match result {
        Ok(_) => Ok(Json(SpawnResponse {
            id: req.id,
            ok: true,
        })),
        Err(e) => Err(WebError::Internal(format!("Failed to spawn PTY: {}", e))),
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
    let ok = state.pty_mgr.write(&req.id, data).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// Resize a web-owned PTY.
pub async fn pty_resize(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyResizeRequest>,
) -> WebResult<impl IntoResponse> {
    let rows = req.rows.clamp(1, 500);
    let cols = req.cols.clamp(1, 500);
    let ok = state.pty_mgr.resize(&req.id, rows, cols).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// Kill a web-owned PTY.
pub async fn pty_kill(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyKillRequest>,
) -> WebResult<impl IntoResponse> {
    let ok = state.pty_mgr.kill(&req.id).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// List active web PTY IDs.
pub async fn pty_list(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let ids = state.pty_mgr.list().await;
    Ok(Json(ids))
}
