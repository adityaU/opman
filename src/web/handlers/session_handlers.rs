//! Session message, command, provider, and permission/question handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::api::ApiClient;
use crate::app::base_url;

/// Query parameters for paginated message fetching.
#[derive(serde::Deserialize)]
pub struct MessagePageQuery {
    /// Maximum number of messages to return. Omit or 0 for all.
    pub limit: Option<usize>,
    /// Only return messages created **before** this Unix-ms timestamp (exclusive).
    /// Used for "load older" pagination — pass the oldest timestamp from the
    /// previous page to fetch the preceding chunk.
    pub before: Option<u64>,
}

/// GET /api/session/:id/messages — fetch messages for a session.
///
/// Supports optional pagination via query parameters:
///   - `?limit=N`             — return only the N most recent messages
///   - `?before=TIMESTAMP`    — return messages before this Unix-ms timestamp
///   - `?limit=N&before=T`    — load N messages before timestamp T
///
/// Response: `{ "messages": [...], "has_more": bool, "total": usize }`
/// Messages are sorted by creation time (ascending — oldest first within the page).
pub async fn get_session_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Query(page): Query<MessagePageQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
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

    // Normalise the response into a flat Vec — upstream may return an array
    // or an object keyed by message ID.
    let mut all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    // Sort by info.time.created to ensure chronological order.
    all_messages.sort_by(|a, b| {
        let time_a = a.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
        let time_b = b.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
        time_a.cmp(&time_b)
    });

    let total = all_messages.len();

    // Apply pagination: filter by `before` timestamp, then take last `limit`.
    let limit = page.limit.unwrap_or(0);

    if limit > 0 || page.before.is_some() {
        // Filter by `before` — keep only messages with created < before
        if let Some(before_ts) = page.before {
            all_messages.retain(|m| {
                let ts = m.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
                ts < before_ts
            });
        }

        let filtered_count = all_messages.len();
        let effective_limit = if limit > 0 { limit } else { filtered_count };

        // Take only the last `limit` messages (most recent within the filtered set)
        let has_more = filtered_count > effective_limit;
        if has_more {
            all_messages = all_messages.split_off(filtered_count - effective_limit);
        }

        Ok(Json(serde_json::json!({
            "messages": all_messages,
            "has_more": has_more,
            "total": total,
        })))
    } else {
        // No pagination — return everything (backward compatible)
        Ok(Json(serde_json::json!({
            "messages": all_messages,
            "has_more": false,
            "total": total,
        })))
    }
}

/// POST /api/session/:id/message — send a message to a session.
pub async fn send_message(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state.http_client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&req)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        tracing::error!(
            %session_id,
            upstream_status = %status,
            upstream_body = %body,
            "send_message: upstream rejected"
        );
        return Err(WebError::Internal(format!("Upstream {}: {:?}", status, body)));
    }
    Ok(Json(body))
}

/// POST /api/session/:id/abort — abort a running session.
pub async fn abort_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .abort_session(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// DELETE /api/session/:id — delete a session.
pub async fn delete_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state
        .http_client
        .delete(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(StatusCode::OK)
}

/// PATCH /api/session/:id — rename a session (update title).
pub async fn rename_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state
        .http_client
        .patch(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "title": req.title }))
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
}

/// POST /api/session/:id/command — execute a slash command.
pub async fn execute_command(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<ExecuteCommandRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let result = client
        .execute_session_command(
            &base,
            &dir,
            &session_id,
            &req.command,
            &req.arguments,
            req.model.as_deref(),
        )
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(result))
}

/// GET /api/providers — fetch available providers and models.
pub async fn get_providers(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let providers = client
        .fetch_providers(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(providers))
}

/// GET /api/commands — list available slash commands.
pub async fn get_commands(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let cmds = client
        .list_commands(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(cmds))
}

/// POST /api/permission/:id/reply — reply to a permission request.
pub async fn reply_permission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<PermissionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_permission(&base, &dir, &request_id, &req.reply)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// POST /api/question/:id/reply — reply to a question.
pub async fn reply_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<QuestionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_question(&base, &dir, &request_id, &req.answers)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// GET /api/pending — return pending permissions and questions across all sessions.
pub async fn get_pending(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let ws = state.web_state.inner.read().await;
    let permissions: Vec<&serde_json::Value> = ws.pending_permissions.values().collect();
    let questions: Vec<&serde_json::Value> = ws.pending_questions.values().collect();
    Ok(Json(serde_json::json!({
        "permissions": permissions,
        "questions": questions,
    })))
}

/// POST /api/session/:id/mark_seen — clear unseen state for a session.
pub async fn mark_session_seen(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    state.web_state.mark_session_seen(&session_id).await;
    Ok(StatusCode::OK)
}
