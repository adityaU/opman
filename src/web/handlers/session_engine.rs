//! Recording what a session is configured to run as.
//!
//! The engines own these values and persist them; this endpoint exists so a choice becomes
//! theirs the moment the user makes it, rather than on the next send. A user who picks a
//! model and then switches sessions has sent nothing, and before this the pick went
//! nowhere — which is what made the composer look like it forgot.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::app::EngineChoices;
use crate::runner::RunnerKind;

/// PATCH /api/session/:id/engine — set the session's model, agent, effort or permission.
///
/// Only the fields present are applied; the rest are left as they were, so the composer
/// can send one chip's change without restating the other three.
pub async fn set_session_engine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(session_id): Path<String>,
    Json(choices): Json<EngineChoices>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // Same binding-before-routing rule as a send: without it the configure request goes to
    // the default engine, which does not own the session and would answer for nobody.
    if let Some(kind) = state
        .web_state
        .session_runner(&session_id)
        .await
        .and_then(|label| RunnerKind::parse(&label))
    {
        state
            .runner_registry
            .ensure_binding(&session_id, kind, &dir)
            .await;
    }

    let accepted = state
        .runner_registry
        .configure(&session_id, &dir, &choices)
        .await
        .map_err(|error| WebError::Internal(format!("Runner error: {error}")))?;

    // Reflect it locally too. The session list is refreshed from the engines on a poll, so
    // without this the client would read its own change back as the old value for as long
    // as a poll interval and the chip would visibly snap back.
    if accepted {
        state
            .web_state
            .apply_session_engine(&session_id, &choices)
            .await;
    }

    Ok(Json(serde_json::json!({ "accepted": accepted })))
}

#[cfg(test)]
#[path = "session_engine_tests.rs"]
mod session_engine_tests;
