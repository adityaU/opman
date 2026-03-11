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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineTrigger {
    Manual,
    OnSessionIdle,
    DailySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineAction {
    ReviewMission,
    OpenInbox,
    OpenActivityFeed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDefinition {
    pub id: String,
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRunRecord {
    pub id: String,
    pub routine_id: String,
    pub status: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    pub trigger: RoutineTrigger,
    pub action: RoutineAction,
    #[serde(default)]
    pub mission_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
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
    pub mission_id: Option<Option<String>>,
    #[serde(default)]
    pub session_id: Option<Option<String>>,
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
