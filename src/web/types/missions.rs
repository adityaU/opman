//! Mission types — goal-driven session execution loop.
//!
//! A mission is a **goal** attached to a **session**. The session executes
//! toward the goal, then an evaluator (system-prompt-driven, same session)
//! assesses whether the goal has been achieved. If not, the cycle repeats
//! automatically until the goal is met or the mission is cancelled/paused.

use serde::{Deserialize, Serialize};

// ── Mission lifecycle ───────────────────────────────────────────────

/// The lifecycle state of a mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionState {
    /// Created but not yet started.
    Pending,
    /// Currently executing (session is working on the goal).
    Executing,
    /// Evaluator is assessing progress.
    Evaluating,
    /// Execution paused — can be resumed.
    Paused,
    /// Goal was achieved.
    Completed,
    /// User cancelled the mission.
    Cancelled,
    /// Mission failed (max iterations reached, or unrecoverable error).
    Failed,
}

/// Evaluator verdict after assessing progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalVerdict {
    /// Goal has been achieved.
    Achieved,
    /// Goal not yet achieved; continue executing.
    Continue,
    /// Blocked — cannot proceed without user input.
    Blocked,
    /// Failed — goal is not achievable.
    Failed,
}

/// Record of a single evaluation cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecord {
    /// Which iteration (1-based).
    pub iteration: u32,
    /// The evaluator's verdict.
    pub verdict: EvalVerdict,
    /// Short summary from the evaluator.
    pub summary: String,
    /// What the evaluator says should happen next (if continuing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
    /// Timestamp (RFC3339).
    pub timestamp: String,
}

// ── Core mission struct ─────────────────────────────────────────────

/// A mission: a goal attached to a session, executed in an evaluate-loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    /// The goal to achieve (user-defined).
    pub goal: String,
    /// The session executing this mission.
    pub session_id: String,
    /// The project index this mission belongs to.
    pub project_index: usize,
    /// Current lifecycle state.
    pub state: MissionState,
    /// Current iteration count (starts at 0, incremented each execute cycle).
    pub iteration: u32,
    /// Maximum iterations before auto-failing (0 = unlimited).
    pub max_iterations: u32,
    /// Last evaluator verdict (if any evaluation has occurred).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verdict: Option<EvalVerdict>,
    /// Last evaluator summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_eval_summary: Option<String>,
    /// History of evaluation records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eval_history: Vec<EvalRecord>,
    /// Timestamp when mission was created (RFC3339).
    pub created_at: String,
    /// Timestamp when mission was last updated (RFC3339).
    pub updated_at: String,
}

// ── API request/response types ──────────────────────────────────────

/// Request to create a new mission.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateMissionRequest {
    /// The goal to achieve.
    pub goal: String,
    /// Session to execute in. If not provided, a new session will be created.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Project index. Defaults to active project.
    #[serde(default)]
    pub project_index: Option<usize>,
    /// Max iterations (0 = unlimited). Default: 10.
    #[serde(default)]
    pub max_iterations: Option<u32>,
}

/// Request to update a mission (control actions).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMissionRequest {
    /// Update the goal text.
    #[serde(default)]
    pub goal: Option<String>,
    /// Update max iterations.
    #[serde(default)]
    pub max_iterations: Option<u32>,
}

/// Action to perform on a mission (start, pause, resume, cancel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionAction {
    Start,
    Pause,
    Resume,
    Cancel,
}

/// Request body for `POST /api/missions/{id}/action`.
#[derive(Debug, Clone, Deserialize)]
pub struct MissionActionRequest {
    pub action: MissionAction,
}

/// List response.
#[derive(Debug, Clone, Serialize)]
pub struct MissionsListResponse {
    pub missions: Vec<Mission>,
}
