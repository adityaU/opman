//! Runner selection actions.
//!
//! Selecting a runner is itself a request to start its server. Keeping this as a
//! separate action lets the picker report startup failures without waiting for a
//! later message-send request to discover them.

use axum::extract::{Json, State};
use axum::response::{IntoResponse, Json as ResponseJson};
use serde::Deserialize;

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::ServerState;
use crate::runner::RunnerKind;

#[derive(Deserialize)]
pub(crate) struct RunnerSelection {
    runner: String,
}

/// POST /api/runner/select — start the selected runner slot.
pub(crate) async fn select_runner(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(selection): Json<RunnerSelection>,
) -> WebResult<impl IntoResponse> {
    let Some(kind) = RunnerKind::parse(&selection.runner) else {
        return Err(WebError::BadRequest(format!(
            "Unknown runner: {}",
            selection.runner
        )));
    };

    let outcome = state
        .runner_registry
        .ensure_started(&kind)
        .await
        .map_err(|error| {
            WebError::Internal(format!(
                "Could not start {} runner: {error}",
                kind.display_name()
            ))
        })?;

    Ok(ResponseJson(serde_json::json!({
        "runner": kind.display_name(),
        "status": outcome,
    })))
}
