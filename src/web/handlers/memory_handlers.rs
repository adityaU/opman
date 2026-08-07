//! Personal memory and autonomy handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

// ── Personal Memory ─────────────────────────────────────────────────

/// GET /api/memory — list all personal memory items.
pub async fn list_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let memory = state.web_state.list_personal_memory().await;
    Json(super::super::types::PersonalMemoryListResponse { memory })
}

/// POST /api/memory — create a personal memory item.
pub async fn create_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::CreatePersonalMemoryRequest>,
) -> impl IntoResponse {
    let item = state.web_state.create_personal_memory(req).await;
    (StatusCode::CREATED, Json(item))
}

/// PATCH /api/memory/{memory_id} — update a memory item.
pub async fn update_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(memory_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::UpdatePersonalMemoryRequest>,
) -> impl IntoResponse {
    match state
        .web_state
        .update_personal_memory(&memory_id, req)
        .await
    {
        Some(item) => (StatusCode::OK, Json(item)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/memory/{memory_id} — delete a memory item.
pub async fn delete_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(memory_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_personal_memory(&memory_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// GET /api/autonomy — get autonomy settings.
pub async fn get_autonomy_settings(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    Json(state.web_state.get_autonomy_settings().await)
}

/// POST /api/autonomy — update autonomy settings.
pub async fn update_autonomy_settings(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::UpdateAutonomySettingsRequest>,
) -> impl IntoResponse {
    Json(state.web_state.update_autonomy_settings(req.mode).await)
}

/// GET /api/memory/active — filtered memory for active scope.
pub async fn list_active_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(params): Query<ActiveMemoryQuery>,
) -> impl IntoResponse {
    let memory = state
        .web_state
        .list_active_memory(params.project_index, params.session_id.as_deref())
        .await;
    Json(PersonalMemoryListResponse { memory })
}

#[cfg(test)]
#[path = "memory_handlers_tests.rs"]
mod memory_handlers_tests;
