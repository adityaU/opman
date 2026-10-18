//! Authenticated loopback adapter for the existing Routines scheduler.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::error::{WebError, WebResult};
use super::super::types::*;

#[path = "internal_scheduler_validation.rs"]
mod validation;

#[derive(Deserialize)]
pub struct SchedulerCall {
    project_dir: String,
    #[serde(flatten)]
    operation: Operation,
}

#[derive(Deserialize)]
#[serde(tag = "operation", content = "args", rename_all = "snake_case")]
enum Operation {
    List {
        #[serde(default)]
        all_projects: bool,
    },
    Create(CreateArgs),
    Update {
        routine_id: String,
        #[serde(flatten)]
        changes: UpdateRoutineRequest,
    },
    Delete {
        routine_id: String,
    },
    Run {
        routine_id: String,
    },
}

// Share the existing fields, with MCP-friendly defaults added before creation.
#[derive(Deserialize)]
struct CreateArgs {
    #[serde(flatten)]
    fields: UpdateRoutineRequest,
}

pub async fn internal_scheduler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(call): Json<SchedulerCall>,
) -> WebResult<Json<Value>> {
    let token = headers
        .get("x-internal-token")
        .and_then(|value| value.to_str().ok());
    if state.internal_token.is_empty() || token != Some(state.internal_token.as_str()) {
        return Err(WebError::Unauthorized);
    }
    let project = validation::resolve_project(&state, &call.project_dir).await?;
    let (mut routines, mut runs) = state.web_state.list_routines().await;
    let value = match call.operation {
        Operation::List { all_projects } => {
            if !all_projects {
                routines.retain(|routine| routine.project_index.unwrap_or(0) == project);
                runs.retain(|run| routines.iter().any(|routine| routine.id == run.routine_id));
            }
            json!({ "routines": routines, "runs": runs })
        }
        Operation::Create(args) => {
            let request = creation_request(args.fields, project)?;
            validation::validate_create(&state, &request, project).await?;
            json!(state.web_state.create_routine(request).await)
        }
        Operation::Update {
            routine_id,
            changes,
        } => {
            let current = find_routine(&routines, &routine_id, project)?;
            validation::validate_update(&state, current, &changes, project).await?;
            let routine = state
                .web_state
                .update_routine(&routine_id, changes)
                .await
                .ok_or(WebError::NotFound("routine"))?;
            json!(routine)
        }
        Operation::Delete { routine_id } => {
            find_routine(&routines, &routine_id, project)?;
            if !state.web_state.delete_routine(&routine_id).await {
                return Err(WebError::NotFound("routine"));
            }
            json!({ "deleted": true, "routine_id": routine_id })
        }
        Operation::Run { routine_id } => {
            find_routine(&routines, &routine_id, project)?;
            json!(state
                .web_state
                .execute_routine(&routine_id)
                .await
                .map_err(WebError::BadRequest)?)
        }
    };
    Ok(Json(value))
}

fn find_routine<'a>(
    routines: &'a [RoutineDefinition],
    id: &str,
    project: usize,
) -> WebResult<&'a RoutineDefinition> {
    routines
        .iter()
        .find(|routine| routine.id == id && routine.project_index.unwrap_or(0) == project)
        .ok_or(WebError::NotFound("routine in current project"))
}

fn creation_request(
    fields: UpdateRoutineRequest,
    project: usize,
) -> WebResult<CreateRoutineRequest> {
    let session_id = fields.session_id.flatten();
    let target_mode = fields
        .target_mode
        .flatten()
        .unwrap_or_else(|| match session_id.as_deref() {
            Some(id) if !id.is_empty() => RoutineTargetMode::ExistingSession,
            _ => RoutineTargetMode::NewSession,
        });
    Ok(CreateRoutineRequest {
        name: fields
            .name
            .ok_or_else(|| WebError::BadRequest("name is required".into()))?,
        trigger: fields.trigger.unwrap_or(RoutineTrigger::Scheduled),
        enabled: fields.enabled.unwrap_or(true),
        cron_expr: fields.cron_expr.flatten(),
        timezone: fields.timezone.flatten(),
        target_mode: Some(target_mode),
        session_id,
        project_index: Some(fields.project_index.flatten().unwrap_or(project)),
        prompt: fields.prompt.flatten(),
        provider_id: fields.provider_id.flatten(),
        model_id: fields.model_id.flatten(),
    })
}

#[cfg(test)]
#[path = "internal_scheduler_tests.rs"]
mod tests;
