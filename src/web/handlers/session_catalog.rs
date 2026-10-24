//! Runner commands, providers, and session command execution.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::session_message_read::RunnerQuery;
use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::handlers::common::{default_base_url, resolve_project_dir};
use crate::web::types::*;

use crate::api::interactions::CommandError;
use crate::api::ApiClient;
use crate::runner::RunnerKind;

/// POST /api/session/:id/command — execute a slash command.
///
/// A slash command is a turn, so it routes the way a turn routes: to the runner that owns
/// the session. `GET /api/commands` already answers per runner, and offering a runner's own
/// vocabulary while executing it against the default engine is how a listed command comes
/// back "session not found". Only sessions opman has no binding for fall through to the
/// raw upstream call.
pub async fn execute_command(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<ExecuteCommandRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;

    // Same binding-before-routing rule as `send_message`: recording the session's runner
    // first keeps a command from being answered by the default engine.
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
    if state.runner_registry.has_binding(&session_id).await {
        let result = state
            .runner_registry
            .execute_command(
                &session_id,
                &dir,
                crate::runner::SlashCommand::new(
                    &req.command,
                    &req.arguments,
                    req.model.as_deref(),
                ),
            )
            .await
            .map_err(WebError::runner)?;
        // A command is a turn: mark it dispatched so the session reads as running for the
        // beat before the runner reports the turn it just accepted.
        state.web_state.mark_turn_dispatched(&session_id).await;
        return Ok(Json(result));
    }

    let base = default_base_url(&state).await?;
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
        .map_err(|e| map_command_error(&e))?;
    Ok(Json(result))
}

/// Map an error from an upstream session command into a `WebError`.
///
/// A `CommandError` preserves the upstream HTTP status; anything else is logged
/// and collapsed into a generic internal error.
pub(crate) fn map_command_error(e: &anyhow::Error) -> WebError {
    if let Some(cmd_err) = e.downcast_ref::<CommandError>() {
        let status =
            StatusCode::from_u16(cmd_err.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        WebError::Upstream(status, cmd_err.message.clone())
    } else {
        tracing::error!("Session command failed: {e}");
        WebError::Internal("Command execution failed".into())
    }
}

/// GET /api/providers — fetch available providers and models.
pub async fn get_providers(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<RunnerQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let startup_kind = query
        .runner
        .as_deref()
        .map(parse_runner)
        .transpose()?
        .unwrap_or_else(|| state.runner_registry.default_kind());
    state
        .runner_registry
        .ensure_started(&startup_kind)
        .await
        .map_err(|e| WebError::Internal(format!("Runner startup failed: {e}")))?;

    if let Some(name) = query.runner.as_deref() {
        let runner = parse_runner(name)?;
        let providers = state
            .runner_registry
            .providers(runner, &dir)
            .await
            .map_err(WebError::runner)?;
        return Ok(Json(providers));
    }
    let base = default_base_url(&state).await?;
    let client = ApiClient::with_client(state.http_client.clone());
    let providers = client
        .fetch_providers(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(providers))
}

/// GET /api/commands — the slash commands the runner advertises for this directory.
///
/// Answered by the runner named in `?runner=`, because a slash command is executed by the
/// agent and every agent has a different set: opencode's own command API, claude's
/// `slash_commands`, and whatever an ACP agent reports in `available_commands_update`.
/// Without a runner the request falls back to the default engine.
pub async fn get_commands(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<RunnerQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let startup_kind = query
        .runner
        .as_deref()
        .map(parse_runner)
        .transpose()?
        .unwrap_or_else(|| state.runner_registry.default_kind());
    state
        .runner_registry
        .ensure_started(&startup_kind)
        .await
        .map_err(|e| WebError::Internal(format!("Runner startup failed: {e}")))?;
    if let Some(name) = query.runner.as_deref() {
        let runner = parse_runner(name)?;
        let commands = state
            .runner_registry
            .commands(runner, &dir)
            .await
            .map_err(WebError::runner)?;
        return Ok(Json(commands));
    }
    let base = default_base_url(&state).await?;
    let client = ApiClient::with_client(state.http_client.clone());
    let cmds = client
        .list_commands(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(cmds))
}

/// Resolve a runner name from a query string, or reject the request.
fn parse_runner(name: &str) -> WebResult<crate::runner::RunnerKind> {
    crate::runner::RunnerKind::parse(name)
        .ok_or_else(|| WebError::BadRequest(format!("Unknown runner: {name}")))
}
