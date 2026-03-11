//! Client presence types.

use serde::{Deserialize, Serialize};

/// Represents a single connected client's presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPresence {
    /// Unique client identifier (random per tab/connection).
    pub client_id: String,
    /// "web" or "tui".
    pub interface_type: String,
    /// Which session this client is currently focused on (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_session: Option<String>,
    /// ISO 8601 timestamp of last heartbeat.
    pub last_seen: String,
}

/// Snapshot of all connected clients — broadcast on presence changes.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceSnapshot {
    pub clients: Vec<ClientPresence>,
}

/// Request body for registering/updating presence.
#[derive(Debug, Clone, Deserialize)]
pub struct PresenceRegisterRequest {
    pub client_id: String,
    pub interface_type: String,
    #[serde(default)]
    pub focused_session: Option<String>,
}

/// Request body for deregistering presence.
#[derive(Debug, Clone, Deserialize)]
pub struct PresenceDeregisterRequest {
    pub client_id: String,
}

/// Response for `GET /api/presence`.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceResponse {
    pub clients: Vec<ClientPresence>,
}
