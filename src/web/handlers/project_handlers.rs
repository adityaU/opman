//! Project management, session selection, directory browsing, and panel handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use crate::app::base_url;

pub async fn switch_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SwitchProjectRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.switch_project(req.index).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project index".into()))
    }
}

pub async fn select_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SelectSessionRequest>,
) -> WebResult<impl IntoResponse> {
    if state
        .web_state
        .select_session(req.project_idx, req.session_id)
        .await
    {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project or session".into()))
    }
}

/// Record what a just-created session should run as, with the runner that owns it.
///
/// Best-effort: an engine with no configure route answers `false`, and a session that runs on
/// its engine's defaults is not a failure worth refusing to create it over.
async fn seed_engine(
    state: &ServerState,
    session_id: &str,
    runner: crate::runner::RunnerKind,
    dir: &str,
    choices: &crate::app::EngineChoices,
) {
    if choices.is_empty() {
        return;
    }
    state
        .runner_registry
        .ensure_binding(session_id, runner, dir)
        .await;
    let _ = state
        .runner_registry
        .configure(session_id, dir, choices)
        .await;
}

pub async fn new_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<NewSessionRequest>,
) -> WebResult<impl IntoResponse> {
    // Resolve the project directory for the opencode server header.
    let dir = state
        .web_state
        .get_project_working_dir(req.project_idx)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .ok_or(WebError::BadRequest("Invalid project index".into()))?;

    if let Some(runner) = req.runner {
        let runner_name = runner.display_name().into_owned();
        let session = state
            .runner_registry
            .create_session(runner.clone(), &dir, "")
            .await
            .map_err(|e| WebError::Internal(format!("Runner error: {e}")))?;
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        // Configure before the row is published: the session list is what the composer reads
        // back, and a row that briefly reports the engine's default would overwrite the very
        // choice this send is about to make.
        seed_engine(&state, &session.id, runner, &dir, &req.engine).await;
        state
            .web_state
            .add_and_activate_session(
                req.project_idx,
                crate::app::SessionInfo {
                    id: session.id.clone(),
                    title: session.title,
                    directory: dir,
                    engine: req.engine,
                    time: crate::app::SessionTime {
                        created: now,
                        updated: now,
                    },
                    ..Default::default()
                },
            )
            .await;
        state
            .web_state
            .set_session_runner(&session.id, &runner_name)
            .await;
        return Ok(Json(NewSessionResponse {
            session_id: session.id,
        }));
    }

    // Create the session synchronously via the opencode server API.
    let base = base_url().to_string();
    let resp = state
        .http_client
        .post(format!("{}/session", base))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }

    // Parse session info from the response.
    let session_info: crate::app::SessionInfo = serde_json::from_value(body.clone())
        .map_err(|e| WebError::Internal(format!("Failed to parse session info: {e}")))?;

    let session_id = session_info.id.clone();

    // Add the new session to web_state and set it as active.
    state
        .web_state
        .add_and_activate_session(req.project_idx, session_info)
        .await;

    Ok(Json(NewSessionResponse { session_id }))
}

/// POST /api/project/add — add a new project by directory path.
pub async fn add_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<AddProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state
        .web_state
        .add_project(&req.path, req.name.as_deref())
        .await
    {
        Ok((index, name)) => Ok(Json(AddProjectResponse { index, name })),
        Err(msg) => Err(WebError::BadRequest(msg)),
    }
}

/// POST /api/project/remove — remove a project by index.
pub async fn remove_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<RemoveProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.remove_project(req.index).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(msg) => Err(WebError::BadRequest(msg)),
    }
}

pub async fn toggle_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<TogglePanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.toggle_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}

pub async fn focus_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FocusPanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.focus_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}

#[cfg(test)]
#[path = "project_handlers_tests.rs"]
mod project_handlers_tests;

#[cfg(test)]
#[path = "project_handlers_upstream_tests.rs"]
mod project_handlers_upstream_tests;
