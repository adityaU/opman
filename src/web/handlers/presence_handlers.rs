//! Client presence handlers (session continuity).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::types::*;

// ── Session Continuity: Presence ────────────────────────────────────

/// GET /api/presence — get current connected clients.
pub async fn get_presence(State(state): State<ServerState>, _auth: AuthUser) -> impl IntoResponse {
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
