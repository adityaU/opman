//! Multi-session dashboard: overview, tree, presence, activity, and missions.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

/// GET /api/sessions/overview — flat list of all sessions across all projects
/// with status, cost, and timing info.
pub async fn sessions_overview(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let overview = state.web_state.get_sessions_overview().await;
    Json(overview)
}

/// GET /api/sessions/tree — hierarchical parent/child session tree.
pub async fn sessions_tree(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let tree = state.web_state.get_sessions_tree().await;
    Json(tree)
}

// ── Session Continuity: Presence + Activity ─────────────────────────

/// GET /api/presence — get current connected clients.
pub async fn get_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let snapshot = state.web_state.get_presence().await;
    Json(super::super::types::PresenceResponse {
        clients: snapshot.clients,
    })
}

/// POST /api/presence — register or update client presence.
pub async fn register_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::PresenceRegisterRequest>,
) -> impl IntoResponse {
    let now = chrono::Utc::now().to_rfc3339();
    let presence = super::super::types::ClientPresence {
        client_id: req.client_id,
        interface_type: req.interface_type,
        focused_session: req.focused_session,
        last_seen: now,
    };
    state.web_state.register_presence(&presence).await;
    StatusCode::OK
}

/// DELETE /api/presence — deregister client presence.
pub async fn deregister_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::PresenceDeregisterRequest>,
) -> impl IntoResponse {
    state.web_state.deregister_presence(&req.client_id).await;
    StatusCode::OK
}

/// Query for activity feed endpoint.
#[derive(serde::Deserialize)]
pub struct ActivityFeedQuery {
    pub session_id: String,
}

/// GET /api/activity — get recent activity events for a session.
pub async fn get_activity_feed(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(params): Query<ActivityFeedQuery>,
) -> impl IntoResponse {
    let events = state.web_state.get_activity_feed(&params.session_id).await;
    Json(super::super::types::ActivityFeedResponse {
        session_id: params.session_id,
        events,
    })
}

// ── Missions ────────────────────────────────────────────────────────

/// GET /api/missions — list all saved missions.
pub async fn list_missions(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let missions = state.web_state.list_missions().await;
    Json(super::super::types::MissionsListResponse { missions })
}

/// POST /api/missions — create a mission.
pub async fn create_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::super::types::CreateMissionRequest>,
) -> impl IntoResponse {
    let mission = state.web_state.create_mission(req).await;
    (StatusCode::CREATED, Json(mission))
}

/// PATCH /api/missions/{mission_id} — update an existing mission.
pub async fn update_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(mission_id): axum::extract::Path<String>,
    Json(req): Json<super::super::types::UpdateMissionRequest>,
) -> impl IntoResponse {
    match state.web_state.update_mission(&mission_id, req).await {
        Some(mission) => (StatusCode::OK, Json(mission)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/missions/{mission_id} — delete a mission.
pub async fn delete_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(mission_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_mission(&mission_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

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
    match state.web_state.update_personal_memory(&memory_id, req).await {
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
