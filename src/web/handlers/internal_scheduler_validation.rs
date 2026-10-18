use std::path::Path;

use crate::web::error::{WebError, WebResult};
use crate::web::types::*;

pub(super) async fn resolve_project(state: &ServerState, directory: &str) -> WebResult<usize> {
    if !Path::new(directory).is_absolute() {
        return invalid("project_dir must be an absolute project path");
    }
    state
        .web_state
        .all_project_paths()
        .await
        .iter()
        .position(|path| Path::new(path) == Path::new(directory))
        .ok_or_else(|| WebError::BadRequest("project_dir is not an opman project".into()))
}

pub(super) async fn validate_create(
    state: &ServerState,
    request: &CreateRoutineRequest,
    project: usize,
) -> WebResult<()> {
    validate_fields(
        &request.name,
        &request.trigger,
        request.cron_expr.as_deref(),
        request.timezone.as_deref(),
        request.prompt.as_deref(),
    )?;
    validate_project(state, request.project_index, project).await?;
    validate_target(
        state,
        project,
        request.target_mode.as_ref(),
        request.session_id.as_deref(),
    )
    .await
}

pub(super) async fn validate_update(
    state: &ServerState,
    current: &RoutineDefinition,
    changes: &UpdateRoutineRequest,
    project: usize,
) -> WebResult<()> {
    validate_fields(
        changes.name.as_deref().unwrap_or(&current.name),
        changes.trigger.as_ref().unwrap_or(&current.trigger),
        changes
            .cron_expr
            .as_ref()
            .unwrap_or(&current.cron_expr)
            .as_deref(),
        changes
            .timezone
            .as_ref()
            .unwrap_or(&current.timezone)
            .as_deref(),
        changes
            .prompt
            .as_ref()
            .unwrap_or(&current.prompt)
            .as_deref(),
    )?;
    validate_project(
        state,
        changes.project_index.unwrap_or(current.project_index),
        project,
    )
    .await?;
    validate_target(
        state,
        project,
        changes
            .target_mode
            .as_ref()
            .unwrap_or(&current.target_mode)
            .as_ref(),
        changes
            .session_id
            .as_ref()
            .unwrap_or(&current.session_id)
            .as_deref(),
    )
    .await
}

async fn validate_project(
    state: &ServerState,
    project: Option<usize>,
    expected: usize,
) -> WebResult<()> {
    if project.unwrap_or(0) != expected {
        return invalid("project_index must match project_dir");
    }
    if state
        .web_state
        .get_project_working_dir(project.unwrap_or(0))
        .await
        .is_none()
    {
        return invalid("project_index is not an opman project");
    }
    Ok(())
}

async fn validate_target(
    state: &ServerState,
    project: usize,
    target: Option<&RoutineTargetMode>,
    session_id: Option<&str>,
) -> WebResult<()> {
    if matches!(target, Some(RoutineTargetMode::NewSession)) {
        return Ok(());
    }
    let Some(id) = session_id.filter(|id| !id.trim().is_empty()) else {
        return invalid("session_id is required for existing_session mode");
    };
    let Some((_, _, sessions)) = state.web_state.get_project_sessions(project).await else {
        return invalid("project_dir is not an opman project");
    };
    if sessions.iter().any(|(session, _)| session == id) {
        return Ok(());
    }
    invalid("session_id does not belong to the current project")
}

fn validate_fields(
    name: &str,
    trigger: &RoutineTrigger,
    cron: Option<&str>,
    timezone: Option<&str>,
    prompt: Option<&str>,
) -> WebResult<()> {
    if name.trim().is_empty() {
        return invalid("name must not be empty");
    }
    if prompt.is_none_or(|text| text.trim().is_empty()) {
        return invalid("prompt is required and must not be empty");
    }
    if let Some(timezone) = timezone {
        timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| WebError::BadRequest("timezone must be an IANA timezone".into()))?;
    }
    if !matches!(trigger, RoutineTrigger::Scheduled) {
        return Ok(());
    }
    let cron = cron.ok_or_else(|| {
        WebError::BadRequest("cron_expr is required for scheduled routines".into())
    })?;
    if crate::web::web_state::WebStateHandle::next_routine_run(cron, timezone).is_none() {
        return invalid("cron_expr is invalid or has no future run");
    }
    Ok(())
}

fn invalid<T>(message: &str) -> WebResult<T> {
    Err(WebError::BadRequest(message.into()))
}
