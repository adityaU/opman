//! `GET`/`PUT /api/runner-limits` — where a session goes when its runner runs low.
//!
//! Stored by the server, not the browser: the automatic switch has to work for a session
//! left running with no tab open, and every device should see the same list.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use super::super::super::auth::AuthUser;
use super::super::super::types::{ServerState, WebEvent};
use crate::runner_limits::{settings, LimitSettings};

pub(crate) async fn get_runner_limits(_auth: AuthUser) -> Json<LimitSettings> {
    Json(settings::global().get().as_ref().clone())
}

/// Replace the settings. The body is validated by its types: an unknown policy or a
/// threshold outside 1–99 is rejected before anything is written.
pub(crate) async fn put_runner_limits(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(next): Json<LimitSettings>,
) -> Result<Json<LimitSettings>, (StatusCode, String)> {
    next.validate()
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let saved = settings::global()
        .set(next)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}")))?;
    // Re-read the accounts now, so a policy just turned on acts within seconds rather
    // than at the next tick of the background reading.
    let client = state.http_client.clone();
    tokio::spawn(async move {
        let accounts = crate::usage::global().accounts(&client, false).await;
        crate::runner_limits::watch::global().store(accounts);
    });
    let _ = state.event_tx.send(WebEvent::RunnerLimitsChanged);
    Ok(Json(saved.as_ref().clone()))
}
