//! `/api/runners/settings` — switch the built-in runners on and off and pick the default.
//!
//! Every change answers with the whole view again, and tells every open client: the set of
//! runners a picker may offer just changed, and so may the runner new sessions go to.

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use super::super::super::auth::AuthUser;
use super::super::super::error::{WebError, WebResult};
use super::super::super::types::{ServerState, WebEvent};
use crate::runner::RunnerKind;
use crate::runner_settings::{self, BuiltinRunner, RunnersView};

fn settings() -> WebResult<&'static std::sync::Arc<runner_settings::RunnerSettings>> {
    runner_settings::global().ok_or(WebError::ServerUnavailable)
}

/// Re-read by every client: the runner list and the default may both have moved.
async fn announce(state: &ServerState) {
    let default = state.runner_registry.default_kind();
    state
        .web_state
        .set_default_runner(&default.display_name())
        .await;
    let _ = state.event_tx.send(WebEvent::AcpAgentsChanged);
    let _ = state.event_tx.send(WebEvent::StateChanged);
}

pub(crate) async fn get_runner_settings(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<Json<RunnersView>> {
    Ok(Json(settings()?.view(&state.runner_registry)))
}

#[derive(Deserialize)]
pub(crate) struct EnabledBody {
    enabled: bool,
}

pub(crate) async fn put_runner_enabled(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<EnabledBody>,
) -> WebResult<Json<RunnersView>> {
    let runner = BuiltinRunner::parse(&id)
        .ok_or_else(|| WebError::BadRequest(format!("{id} is not a built-in runner")))?;
    let settings = settings()?;
    settings
        .set_enabled(&state.runner_registry, runner, body.enabled)
        .map_err(|error| WebError::BadRequest(format!("{error:#}")))?;
    announce(&state).await;
    Ok(Json(settings.view(&state.runner_registry)))
}

#[derive(Deserialize)]
pub(crate) struct DefaultBody {
    runner: String,
}

pub(crate) async fn put_default_runner(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(body): Json<DefaultBody>,
) -> WebResult<Json<RunnersView>> {
    let kind = RunnerKind::parse(&body.runner)
        .ok_or_else(|| WebError::BadRequest(format!("unknown runner {}", body.runner)))?;
    let settings = settings()?;
    settings
        .set_default(&state.runner_registry, kind)
        .map_err(|error| WebError::BadRequest(format!("{error:#}")))?;
    announce(&state).await;
    Ok(Json(settings.view(&state.runner_registry)))
}
