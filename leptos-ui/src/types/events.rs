//! SSE event types — matches TypeScript `OpenCodeEvent` and SSE types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub properties: serde_json::Value,
}

/// Watcher status pushed via SSE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub session_id: String,
    pub action: String,
    pub idle_since_secs: Option<u64>,
}

/// MCP agent activity event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAgentActivity {
    pub tool: String,
    pub active: bool,
}

/// MCP editor open event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEditorOpen {
    pub path: String,
    pub line: Option<u32>,
}
