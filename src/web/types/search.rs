//! Cross-session search types.

use serde::Serialize;

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

#[cfg(test)]
#[path = "search_tests.rs"]
mod search_tests;
