//! Autonomy controls, routines, delegation board, and workspace snapshot types.

use serde::{Deserialize, Serialize};

// ─── Autonomy Controls ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    Observe,
    Nudge,
    Continue,
    Autonomous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomySettings {
    pub mode: AutonomyMode,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAutonomySettingsRequest {
    pub mode: AutonomyMode,
}

// ─── Routines ────────────────────────────────────────────

/// How a routine is triggered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineTrigger {
    /// User clicks "Run" (or API call).
    Manual,
    /// Fires on a cron schedule (backend scheduler).
    Scheduled,
    /// Fires when the bound session goes idle.
    OnSessionIdle,
    /// Legacy — runs once per day (frontend-driven).
    DailySummary,
}

/// What the routine does when it fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineAction {
    /// Send a predefined message to a session.
    SendMessage,
    /// Legacy: open the missions review panel.
    ReviewMission,
    /// Legacy: open the inbox panel.
    OpenInbox,
    /// Legacy: open the activity feed panel.
    OpenActivityFeed,
}

/// Where the routine's message should be sent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineTargetMode {
    /// Send to an existing session (session_id must be set).
    ExistingSession,
    /// Create a new session first, then send.
    NewSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDefinition {
    pub id: String,
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    /// Whether this routine is active (scheduler only fires enabled routines).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Cron expression (5-field). Only meaningful when trigger = Scheduled.
    #[serde(default)]
    pub cron_expr: Option<String>,
    /// IANA timezone for the cron schedule (e.g. "America/New_York").
    #[serde(default)]
    pub timezone: Option<String>,
    /// How to target the session.
    #[serde(default)]
    pub target_mode: Option<RoutineTargetMode>,
    /// Session ID for ExistingSession target mode.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Project index for NewSession target mode.
    #[serde(default)]
    pub project_index: Option<usize>,
    /// The predefined message/prompt to send.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Provider ID to use (e.g. "anthropic" or "openai").
    #[serde(default)]
    pub provider_id: Option<String>,
    /// Model ID to use (e.g. "claude-sonnet-4-20250514").
    #[serde(default)]
    pub model_id: Option<String>,
    /// Legacy: linked mission ID.
    #[serde(default)]
    pub mission_id: Option<String>,
    /// ISO timestamp of last successful run.
    #[serde(default)]
    pub last_run_at: Option<String>,
    /// ISO timestamp of the next scheduled run (computed from cron).
    #[serde(default)]
    pub next_run_at: Option<String>,
    /// Error message from the last failed run, if any.
    #[serde(default)]
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRunRecord {
    pub id: String,
    pub routine_id: String,
    /// "completed", "failed", "running".
    pub status: String,
    pub summary: String,
    /// Session ID that the message was sent to (if applicable).
    #[serde(default)]
    pub target_session_id: Option<String>,
    /// How long the run took in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cron_expr: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub target_mode: Option<RoutineTargetMode>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub project_index: Option<usize>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub mission_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRoutineRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub trigger: Option<RoutineTrigger>,
    #[serde(default)]
    pub action: Option<RoutineAction>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub cron_expr: Option<Option<String>>,
    #[serde(default)]
    pub timezone: Option<Option<String>>,
    #[serde(default)]
    pub target_mode: Option<Option<RoutineTargetMode>>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
    #[serde(default)]
    pub project_index: Option<Option<usize>>,
    #[serde(default)]
    pub prompt: Option<Option<String>>,
    #[serde(default)]
    pub provider_id: Option<Option<String>>,
    #[serde(default)]
    pub model_id: Option<Option<String>>,
    #[serde(default)]
    pub mission_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunRoutineRequest {
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutinesListResponse {
    pub routines: Vec<RoutineDefinition>,
    pub runs: Vec<RoutineRunRecord>,
}

// ─── Delegation Board ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationStatus {
    Planned,
    Running,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedWorkItem {
    pub id: String,
    pub title: String,
    pub assignee: String,
    pub scope: String,
    pub status: DelegationStatus,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub subagent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDelegatedWorkRequest {
    pub title: String,
    pub assignee: String,
    pub scope: String,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub subagent_session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDelegatedWorkRequest {
    #[serde(default)]
    pub status: Option<DelegationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DelegatedWorkListResponse {
    pub items: Vec<DelegatedWorkItem>,
}
