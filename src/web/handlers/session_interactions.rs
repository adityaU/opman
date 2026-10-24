//! Session permissions, questions, and UI callbacks.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::handlers::common::{default_base_url, resolve_project_dir};
use crate::web::types::*;

use crate::api::ApiClient;

/// POST /api/permission/:id/reply — reply to a permission request.
pub async fn reply_permission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<PermissionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .reply_permission(&request_id, &req.reply)
        .await
        .map_err(WebError::runner)?
    {
        return Ok(StatusCode::OK);
    }
    let base = default_base_url(&state).await?;
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_permission(&base, &dir, &request_id, &req.reply)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// POST /api/question/:id/reply — reply to a question.
///
/// The local registry is asked first: a question raised through the `ask` MCP server is
/// held by this process, and fanning it out to the engines would find no owner and then
/// fail against the upstream server.
pub async fn reply_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<QuestionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let Err(answers) = state.ask_pending.resolve(&request_id, req.answers) else {
        return Ok(StatusCode::OK);
    };
    let dir = resolve_project_dir(&state).await?;
    if state
        .runner_registry
        .reply_question(&request_id, &answers)
        .await
        .map_err(WebError::runner)?
    {
        return Ok(StatusCode::OK);
    }
    let base = default_base_url(&state).await?;
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_question(&base, &dir, &request_id, &answers)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// Take down any question the `ask` server raised for a session that is being aborted.
///
/// The engines already do this for their own prompts; a locally-held question has to be
/// cleared here because nothing else knows about it. Leaving one up would invite the user
/// to answer into a turn that has already unwound — and the asker is being killed with the
/// turn, so nothing would receive the answer.
pub(crate) async fn clear_asked_questions(state: &ServerState, session_id: &str, dir: &str) {
    for id in state.ask_pending.clear_session(session_id) {
        state
            .web_state
            .publish_event(
                &serde_json::json!({
                    "type": "question.rejected",
                    "properties": { "id": id, "requestID": id, "sessionID": session_id },
                }),
                dir,
            )
            .await;
    }
}

/// POST /api/question/:id/reject — dismiss a question without answering it.
///
/// The frontend has always called this; until now nothing served it, so dismissing a card
/// left the asker blocked. Rejecting is best-effort by design: a card the user dismisses
/// after its asker already gave up still has to disappear, so an id nobody owns is a
/// success, not a 404.
pub async fn reject_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    if state.ask_pending.dismiss(&request_id) {
        return Ok(StatusCode::OK);
    }
    state
        .runner_registry
        .reject_question(&request_id)
        .await
        .map_err(WebError::runner)?;
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

/// POST /api/session/:id/a2ui/callback — inject an A2UI interaction
/// (button click or form submission) back into the session as a user
/// message so the agent can see and react to it.
pub async fn a2ui_callback(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<A2uiCallbackRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // Format the callback as a structured user message the agent can parse.
    let text = a2ui_callback_text(&req.callback_id, &req.payload);

    let msg_body = serde_json::json!({
        "parts": [{
            "type": "text",
            "text": text,
        }]
    });

    // The session's own runner gets the click. Posting to the default runner would hand a
    // claude session's button press to opencode, which has never heard of it.
    if state
        .runner_registry
        .has_known_session(&session_id, &dir)
        .await
    {
        state
            .runner_registry
            .send_message(&session_id, &dir, None, msg_body)
            .await
            .map_err(WebError::runner)?;
        return a2ui_callback_result(StatusCode::OK, serde_json::Value::Null);
    }
    let base = default_base_url(&state).await?;

    let resp = state
        .http_client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&msg_body)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = if status.is_success() {
        serde_json::Value::Null
    } else {
        resp.json().await.unwrap_or(serde_json::Value::Null)
    };
    a2ui_callback_result(status, body)
}

/// Build the structured user-message text that represents an A2UI callback.
///
/// A null or empty-object payload produces a bare marker line; any other
/// payload is appended as a pretty-printed fenced JSON block.
pub(crate) fn a2ui_callback_text(callback_id: &str, payload: &serde_json::Value) -> String {
    if payload.is_null() || *payload == serde_json::json!({}) {
        format!("[A2UI callback: {}]", callback_id)
    } else {
        let payload_str = serde_json::to_string_pretty(payload).unwrap_or_default();
        format!(
            "[A2UI callback: {}]\n```json\n{}\n```",
            callback_id, payload_str
        )
    }
}

/// Map the A2UI callback upstream response: success → `{ "ok": true }`,
/// otherwise an internal error carrying the upstream status and body.
pub(crate) fn a2ui_callback_result(
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
