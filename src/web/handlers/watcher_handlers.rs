//! Watcher CRUD and watcher session/message handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::app::base_url;

/// GET /api/watchers — list all active watchers with real-time status.
pub async fn list_watchers(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let watchers = state.web_state.list_watchers().await;
    Ok(Json(watchers))
}

/// POST /api/watcher — create or update a watcher for a session.
pub async fn create_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<WatcherConfigRequest>,
) -> WebResult<impl IntoResponse> {
    if req.session_id.is_empty() {
        return Err(WebError::BadRequest("session_id is required".into()));
    }
    if req.continuation_message.trim().is_empty() {
        return Err(WebError::BadRequest("continuation_message is required".into()));
    }
    if req.idle_timeout_secs == 0 {
        return Err(WebError::BadRequest("idle_timeout_secs must be > 0".into()));
    }
    let response = state.web_state.create_watcher(req).await;
    Ok(Json(response))
}

/// DELETE /api/watcher/:session_id — remove a watcher.
pub async fn delete_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.delete_watcher(&session_id).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::NotFound("No watcher found for this session"))
    }
}

/// GET /api/watcher/:session_id — get watcher config and status for a session.
pub async fn get_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.get_watcher(&session_id).await {
        Some(w) => Ok(Json(w)),
        None => Err(WebError::NotFound("No watcher found for this session")),
    }
}

/// GET /api/watcher/sessions — list all sessions formatted for the watcher session picker.
pub async fn get_watcher_sessions(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let sessions = state.web_state.get_watcher_sessions().await;
    Ok(Json(sessions))
}

/// GET /api/watcher/:session_id/messages — fetch user messages from a session
/// for the "re-inject original message" picker.
pub async fn get_watcher_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // Fetch messages from the opencode server
    let resp = state.http_client
        .get(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    // Extract user messages only
    let all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    let mut user_messages: Vec<WatcherMessageEntry> = Vec::new();
    for msg in &all_messages {
        let role = msg.pointer("/info/role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if role != "user" {
            continue;
        }
        // Extract text from parts
        let parts = msg.get("parts").and_then(|v| v.as_array());
        if let Some(parts) = parts {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    if !text.trim().is_empty() {
                        user_messages.push(WatcherMessageEntry {
                            role: "user".to_string(),
                            text: text.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Reverse so most recent is first
    user_messages.reverse();

    Ok(Json(user_messages))
}
