//! Watcher types for session auto-continuation.

use serde::{Deserialize, Serialize};

/// Request to create or update a session watcher.
#[derive(Deserialize, Clone)]
pub struct WatcherConfigRequest {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    #[serde(default)]
    pub include_original: bool,
    pub original_message: Option<String>,
    #[serde(default = "default_hang_message")]
    pub hang_message: String,
    #[serde(default = "default_hang_timeout")]
    pub hang_timeout_secs: u64,
}

fn default_hang_message() -> String {
    "The previous attempt appears to have stalled. Please retry the task.".to_string()
}

fn default_hang_timeout() -> u64 {
    180
}

/// Response for a single watcher entry.
#[derive(Serialize, Clone, Debug)]
pub struct WatcherConfigResponse {
    pub session_id: String,
    pub project_idx: usize,
    pub idle_timeout_secs: u64,
    pub continuation_message: String,
    pub include_original: bool,
    pub original_message: Option<String>,
    pub hang_message: String,
    pub hang_timeout_secs: u64,
    /// Current watcher status: "idle_countdown", "running", "waiting", "inactive"
    pub status: String,
    /// Seconds since session went idle (if in countdown).
    pub idle_since_secs: Option<u64>,
}

/// A list entry for GET /api/watchers.
#[derive(Serialize, Clone)]
pub struct WatcherListEntry {
    pub session_id: String,
    pub session_title: String,
    pub project_name: String,
    pub idle_timeout_secs: u64,
    pub status: String,
    pub idle_since_secs: Option<u64>,
}

/// Session entry for the watcher modal session picker.
#[derive(Serialize, Clone)]
pub struct WatcherSessionEntry {
    pub session_id: String,
    pub title: String,
    pub project_name: String,
    pub project_idx: usize,
    pub is_current: bool,
    pub is_active: bool,
    pub has_watcher: bool,
}

/// SSE event payload for watcher status changes.
#[derive(Clone, Debug, Serialize)]
pub struct WatcherStatusEvent {
    pub session_id: String,
    /// "created", "deleted", "triggered", "countdown", "cancelled"
    pub action: String,
    pub idle_since_secs: Option<u64>,
}

/// A user message from a session for the original-message picker.
#[derive(Serialize, Clone)]
pub struct WatcherMessageEntry {
    pub role: String,
    pub text: String,
}
