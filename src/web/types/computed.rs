//! Computed / intelligence types — inputs and shared enums.
//!
//! These types represent the inputs that the frontend passes to computed
//! intelligence endpoints, plus shared enums used across responses.

use serde::{Deserialize, Serialize};

// ─── Shared enums ───────────────────────────────────────────────────

/// Priority of an inbox item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxItemPriority {
    High,
    Medium,
    Low,
}

/// State of an inbox item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxItemState {
    Unresolved,
    Informational,
}

/// Source category for an inbox item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxItemSource {
    Permission,
    Question,
    Mission,
    Watcher,
    Completion,
}

/// Action the frontend should take when a recommendation is clicked.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationAction {
    OpenInbox,
    OpenMemory,
    #[allow(dead_code)]
    OpenRoutines,
    OpenDelegation,
    OpenWorkspaces,
    #[allow(dead_code)]
    OpenAutonomy,
    SetupDailySummary,
    UpgradeAutonomyNudge,
    SetupDailyCopilot,
}

// ─── Transient input types ──────────────────────────────────────────

/// Transient permission request (passed by frontend from SSE state).
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionInput {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "toolName")]
    pub tool_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub time: f64,
}

/// Transient question request (passed by frontend from SSE state).
#[derive(Debug, Clone, Deserialize)]
pub struct QuestionInput {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub title: String,
    pub time: f64,
}

/// Transient signal (passed by frontend or stored server-side).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignalInput {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub created_at: f64,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Transient watcher status (passed by frontend from SSE state).
#[derive(Debug, Clone, Deserialize)]
pub struct WatcherStatusInput {
    pub session_id: String,
    pub action: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub idle_since_secs: Option<u64>,
}

// ─── Request bodies ─────────────────────────────────────────────────

/// Request body for `POST /api/inbox`.
#[derive(Debug, Clone, Deserialize)]
pub struct InboxRequest {
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
    #[serde(default)]
    pub watcher_status: Option<WatcherStatusInput>,
    #[serde(default)]
    pub signals: Vec<SignalInput>,
}

/// Request body for `POST /api/recommendations`.
#[derive(Debug, Clone, Deserialize)]
pub struct RecommendationsRequest {
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
}

/// Request body for `POST /api/handoff/session`.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionHandoffRequest {
    pub session_id: String,
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
}

/// Request body for `POST /api/resume-briefing`.
#[derive(Debug, Clone, Deserialize)]
pub struct ResumeBriefingRequest {
    #[serde(default)]
    pub active_session_id: Option<String>,
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
    #[serde(default)]
    pub signals: Vec<SignalInput>,
}

/// Request body for `POST /api/daily-summary`.
#[derive(Debug, Clone, Deserialize)]
pub struct DailySummaryRequest {
    pub routine_id: String,
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
    #[serde(default)]
    pub signals: Vec<SignalInput>,
}

/// Request body for `POST /api/signals`.
#[derive(Debug, Clone, Deserialize)]
pub struct AddSignalRequest {
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Request body for `POST /api/assistant-center/stats`.
#[derive(Debug, Clone, Deserialize)]
pub struct AssistantCenterStatsRequest {
    #[serde(default)]
    pub permissions: Vec<PermissionInput>,
    #[serde(default)]
    pub questions: Vec<QuestionInput>,
}

/// Query params for `GET /api/memory/active`.
#[derive(Debug, Clone, Deserialize)]
pub struct ActiveMemoryQuery {
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}
