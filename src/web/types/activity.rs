//! Activity feed and cross-session search types.

use serde::Serialize;

// ── Session Continuity / Activity Feed ──────────────────────────────

/// A single activity event in a session (fine-grained, real-time).
#[derive(Debug, Clone, Serialize)]
pub struct ActivityEventPayload {
    /// Session this activity belongs to.
    pub session_id: String,
    /// Event category: "file_edit", "tool_call", "terminal", "permission", "question", "status".
    pub kind: String,
    /// Human-readable summary of what happened.
    pub summary: String,
    /// Optional detail (file path, tool name, command, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

/// Recent activity events for a session.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityFeedResponse {
    pub session_id: String,
    pub events: Vec<ActivityEventPayload>,
}

// ── Cross-Session Search ────────────────────────────────────────────

/// A single search result — a matching message snippet from a session.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub message_id: String,
    pub role: String,
    /// Text snippet containing the match (truncated).
    pub snippet: String,
    /// Unix timestamp (seconds) of the message.
    pub timestamp: u64,
}

/// Response for GET /api/project/{idx}/search.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResultEntry>,
    pub total: usize,
}
