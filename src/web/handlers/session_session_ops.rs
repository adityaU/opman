//! Session lifecycle and queued-message handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::handlers::common::{default_base_url, resolve_project_dir};
use crate::web::types::*;

use crate::api::ApiClient;

/// POST /api/session/:id/abort — abort a running session.
pub async fn abort_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    // Drop the dispatch grace first: an abort that lands inside it must not be
    // outlived by the mark that was holding the session running.
    state.web_state.mark_turn_settled(&session_id).await;
    super::session_interactions::clear_asked_questions(&state, &session_id, &dir).await;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
    {
        state
            .runner_registry
            .abort(&session_id, &dir)
            .await
            .map_err(WebError::runner)?;
        return Ok(StatusCode::OK);
    }
    let base = default_base_url(&state).await?;
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .abort_session(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// GET /api/session/:id/queue — list queued follow-up prompts.
pub async fn get_session_queue(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::GET, &session_id, None).await
}

/// DELETE /api/session/:id/queue — clear all queued follow-ups.
pub async fn clear_session_queue(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::DELETE, &session_id, None).await
}

/// DELETE /api/session/:id/queue/:index — remove one queued follow-up by index.
pub async fn remove_session_queue_item(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path((session_id, index)): axum::extract::Path<(String, usize)>,
) -> WebResult<impl IntoResponse> {
    proxy_queue(&state, reqwest::Method::DELETE, &session_id, Some(index)).await
}

/// Forward a queue request to the engine and relay its JSON body.
async fn proxy_queue(
    state: &ServerState,
    method: reqwest::Method,
    session_id: &str,
    index: Option<usize>,
) -> WebResult<Json<serde_json::Value>> {
    let dir = resolve_project_dir(state).await?;
    let base = default_base_url(state).await?;
    let path = match index {
        Some(i) => format!("{}/session/{}/queue/{}", base, session_id, i),
        None => format!("{}/session/{}/queue", base, session_id),
    };
    let resp = state
        .http_client
        .request(method, path)
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    map_proxy_json_response(status, body)
}

/// Relay a proxied upstream JSON body, mapping non-2xx statuses to an internal
/// error carrying the upstream status and body. Shared by the queue and rename
/// proxies (identical semantics).
pub(crate) fn map_proxy_json_response(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
}

/// DELETE /api/session/:id — delete a session.
pub async fn delete_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
        && state
            .runner_registry
            .delete(&session_id, &dir)
            .await
            .map_err(WebError::runner)?
    {
        return Ok(StatusCode::OK);
    }
    let base = default_base_url(&state).await?;
    let resp = state
        .http_client
        .delete(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = if status.is_success() {
        serde_json::Value::Null
    } else {
        resp.json().await.unwrap_or(serde_json::Value::Null)
    };
    map_status_only_response(status, body)
}

/// Map a status-only upstream response (delete): success → `200 OK`, otherwise
/// an internal error carrying the upstream status and body.
pub(crate) fn map_status_only_response(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<StatusCode> {
    if !status.is_success() {
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
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
        && state
            .runner_registry
            .rename(&session_id, &req.title, &dir)
            .await
            .map_err(WebError::runner)?
    {
        return Ok(Json(serde_json::json!({ "ok": true, "title": req.title })));
    }
    let base = default_base_url(&state).await?;
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
    map_proxy_json_response(status, body)
}
