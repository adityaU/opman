//! Routines, delegated work, and workspace snapshot handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

/// GET /api/routines — list routines and run history.
pub async fn list_routines(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let (routines, runs) = state.web_state.list_routines().await;
    Json(super::super::types::RoutinesListResponse { routines, runs })
}

/// POST /api/routines — create a routine.
pub async fn create_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::CreateRoutineRequest>,
) -> impl IntoResponse {
    let routine = state.web_state.create_routine(req).await;
    (StatusCode::CREATED, Json(routine))
}

/// PATCH /api/routines/{routine_id} — update a routine.
pub async fn update_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::UpdateRoutineRequest>,
) -> impl IntoResponse {
    match state.web_state.update_routine(&routine_id, req).await {
        Some(routine) => (StatusCode::OK, Json(routine)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/routines/{routine_id} — delete a routine.
pub async fn delete_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_routine(&routine_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// POST /api/routines/{routine_id}/run — record a manual run.
pub async fn run_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::RunRoutineRequest>,
) -> impl IntoResponse {
    Json(
        state
            .web_state
            .record_routine_run(
                &routine_id,
                req.summary.unwrap_or_else(|| "Routine executed manually".to_string()),
            )
            .await,
    )
}

/// GET /api/delegation — list delegated work items.
pub async fn list_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    Json(super::super::types::DelegatedWorkListResponse {
        items: state.web_state.list_delegated_work().await,
    })
}

/// POST /api/delegation — create delegated work item.
pub async fn create_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::CreateDelegatedWorkRequest>,
) -> impl IntoResponse {
    let item = state.web_state.create_delegated_work(req).await;
    (StatusCode::CREATED, Json(item))
}

/// PATCH /api/delegation/{item_id} — update delegated work item.
pub async fn update_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(item_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::UpdateDelegatedWorkRequest>,
) -> impl IntoResponse {
    match state.web_state.update_delegated_work(&item_id, req).await {
        Some(item) => (StatusCode::OK, Json(item)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/delegation/{item_id} — delete delegated work item.
pub async fn delete_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(item_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_delegated_work(&item_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// ── Workspace Snapshots ─────────────────────────────────────────────

/// GET /api/workspaces — list all saved workspace snapshots.
pub async fn list_workspaces(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let workspaces = state.web_state.list_workspaces().await;
    Json(super::super::types::WorkspacesListResponse { workspaces })
}

/// POST /api/workspaces — save (upsert) a workspace snapshot.
pub async fn save_workspace(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::SaveWorkspaceRequest>,
) -> impl IntoResponse {
    state.web_state.save_workspace(req.snapshot).await;
    StatusCode::OK
}

/// Query param for workspace deletion by name.
#[derive(serde::Deserialize)]
pub struct WorkspaceDeleteQuery {
    pub name: String,
}

/// DELETE /api/workspaces — delete a workspace snapshot by name.
pub async fn delete_workspace(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(params): Query<WorkspaceDeleteQuery>,
) -> impl IntoResponse {
    if state.web_state.delete_workspace(&params.name).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}
