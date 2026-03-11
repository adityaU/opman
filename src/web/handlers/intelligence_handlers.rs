//! Computed intelligence API handlers.
//!
//! These endpoints return server-computed derived views. The frontend
//! passes transient SSE data (permissions, questions, signals, watcher
//! status) in POST request bodies; the backend reads its own persisted
//! state and computes the result server-side.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

/// POST /api/inbox — server-computed unified inbox.
pub async fn compute_inbox(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<InboxRequest>,
) -> impl IntoResponse {
    let items = state.web_state.build_inbox(req).await;
    Json(InboxResponse { items })
}

/// POST /api/recommendations — server-computed recommendations.
pub async fn compute_recommendations(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<RecommendationsRequest>,
) -> impl IntoResponse {
    let recommendations = state.web_state.build_recommendations(req).await;
    Json(RecommendationsResponse { recommendations })
}

/// POST /api/handoff/session — session handoff brief.
pub async fn compute_session_handoff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SessionHandoffRequest>,
) -> impl IntoResponse {
    match state.web_state.build_session_handoff(req).await {
        Some(brief) => Json(serde_json::to_value(brief).unwrap()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// POST /api/resume-briefing — server-computed resume briefing.
pub async fn compute_resume_briefing(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<ResumeBriefingRequest>,
) -> impl IntoResponse {
    match state.web_state.build_resume_briefing(req).await {
        Some(briefing) => Json(serde_json::to_value(briefing).unwrap()).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// POST /api/daily-summary — server-computed daily summary.
pub async fn compute_daily_summary(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<DailySummaryRequest>,
) -> impl IntoResponse {
    let summary = state.web_state.build_daily_summary(req).await;
    Json(DailySummaryResponse { summary })
}

/// GET /api/signals — list stored signals.
pub async fn list_signals(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let signals = state.web_state.list_signals().await;
    Json(SignalsResponse { signals })
}

/// POST /api/signals — add a new signal.
pub async fn add_signal(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<AddSignalRequest>,
) -> impl IntoResponse {
    let signal = state.web_state.add_signal(req).await;
    (StatusCode::CREATED, Json(signal))
}

/// POST /api/assistant-center/stats — server-computed dashboard stats.
pub async fn compute_assistant_stats(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<AssistantCenterStatsRequest>,
) -> impl IntoResponse {
    let stats = state.web_state.build_assistant_stats(req).await;
    Json(stats)
}

/// GET /api/workspace-templates — built-in workspace templates.
pub async fn list_workspace_templates(
    State(_state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let templates = super::super::web_state::WebStateHandle::workspace_templates();
    Json(WorkspaceTemplatesResponse { templates })
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
