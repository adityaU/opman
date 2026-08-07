//! Routine handlers.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

/// GET /api/routines — list routines and run history.
pub async fn list_routines(State(state): State<ServerState>, _auth: AuthUser) -> impl IntoResponse {
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

/// POST /api/routines/{routine_id}/run — execute a routine (send message or record manual run).
pub async fn run_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::RunRoutineRequest>,
) -> impl IntoResponse {
    // If a summary is provided (legacy client-side execution), just record the run
    if let Some(summary) = req.summary {
        let run = state
            .web_state
            .record_routine_run(&routine_id, summary, None, None, "completed")
            .await;
        return (StatusCode::OK, Json(run)).into_response();
    }

    // Otherwise, execute the routine server-side
    match state.web_state.execute_routine(&routine_id).await {
        Ok(run) => (StatusCode::OK, Json(run)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
