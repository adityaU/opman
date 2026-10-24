//! Session message delivery and opening instructions.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::handlers::common::{default_base_url, resolve_project_dir};
use crate::web::types::*;

use crate::runner::RunnerKind;

#[path = "session_handoff.rs"]
mod handoff;
#[path = "session_instruction_delivery.rs"]
mod instruction_delivery;

/// POST /api/session/:id/message — send a message to a session.
pub async fn send_message(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(mut req): Json<SendMessageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // The session's own runner, when we know it. Recording the binding before
    // routing keeps an unnamed runner from being read as "switch to the default
    // runner", which would fork the session into a handoff.
    let owner = state
        .web_state
        .session_runner(&session_id)
        .await
        .and_then(|label| RunnerKind::parse(&label));
    if let Some(ref kind) = owner {
        state
            .runner_registry
            .ensure_binding(&session_id, kind.clone(), &dir)
            .await;
    }
    // `runner` is present only when the caller wants that runner specifically —
    // a new session's first turn, or a deliberate switch. A session with a known
    // owner goes through the registry regardless, so an omitted runner still
    // reaches the engine holding the transcript and gets that engine's own
    // effort/permission mapping. Only sessions opman knows nothing about fall
    // through to the raw upstream call.
    let route_through_registry = req.runner.is_some() || owner.is_some();

    // Session instructions open a session, they do not accompany every turn.
    // A requested runner that differs from the session's owner is about to mint
    // a handoff session, and that handoff message is the new session's first
    // turn — so it carries the block even when the session being left already
    // had it.
    let opens_handoff = req
        .runner
        .as_ref()
        .zip(owner.as_ref())
        .is_some_and(|(requested, current)| requested != current);
    let instruction_claim =
        instruction_delivery::prepare(&state, &session_id, &dir, &mut req.parts, opens_handoff)
            .await;

    // The session is running from here on. No runner reports a turn before it
    // has accepted it, and an interface that reads idle for that beat looks
    // like the prompt was dropped. The mark expires on its own, so a send that
    // never reaches a runner cannot strand the session as running.
    state.web_state.mark_turn_dispatched(&session_id).await;

    if route_through_registry {
        let requested = req.runner.clone();
        let request_body = match serde_json::to_value(&req) {
            Ok(body) => body,
            Err(error) => {
                return Err(WebError::Internal(format!("Invalid message: {error}")));
            }
        };
        let outcome = match state
            .runner_registry
            .send_message(&session_id, &dir, requested, request_body)
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Some(claim) = instruction_claim {
                    claim.finish(None, false).await;
                }
                return Err(WebError::runner(error));
            }
        };
        if let Some(claim) = instruction_claim {
            claim.finish(Some(&outcome.session_id), true).await;
        }
        state
            .web_state
            .set_session_runner(&outcome.session_id, &outcome.runner.display_name())
            .await;

        handoff::publish(&state, &session_id, &dir, &outcome).await;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "session_id": outcome.session_id,
            "runner": outcome.runner,
            "switched": outcome.switched,
            "response": outcome.response,
        })));
    }

    let base = match default_base_url(&state).await {
        Ok(base) => base,
        Err(error) => return Err(error),
    };
    let resp = match state
        .http_client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&req)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return Err(WebError::Internal(format!("Upstream error: {error}"))),
    };
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    let result = map_send_message_response(&session_id, status, body);
    if let Some(claim) = instruction_claim {
        claim.finish(None, result.is_ok()).await;
    }
    result
}

/// Map a `send_message` upstream response into the handler result.
///
/// On success the raw body is relayed verbatim; on failure the upstream status
/// and body are logged and surfaced as an internal error.
pub(crate) fn map_send_message_response(
    session_id: &str,
    status: StatusCode,
    body: serde_json::Value,
) -> WebResult<Json<serde_json::Value>> {
    if !status.is_success() {
        tracing::error!(
            %session_id,
            upstream_status = %status,
            upstream_body = %body,
            "send_message: upstream rejected"
        );
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
}
