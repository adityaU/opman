//! Pipeline-launch domain types: a staged, multi-session run of a kanban task
//! where each lane executes in its own session, chained by output.

use serde::{Deserialize, Serialize};

/// One stage of a pipeline-mode launch: a lane that runs in its own session,
/// seeded with the previous stage's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub lane_id: String,
    /// The session created for this stage (None until the stage starts).
    #[serde(default)]
    pub session_id: Option<String>,
    /// "pending" | "running" | "done" | "failed".
    pub status: String,
    /// Captured textual output of the stage (the agent's final message).
    #[serde(default)]
    pub output: Option<String>,
}

/// A staged (pipeline) launch: each lane is a separate session, chained by output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub task_id: String,
    pub stages: Vec<PipelineStage>,
    /// Index into `stages` of the stage currently running (or last run).
    pub current_index: usize,
    /// "running" | "done" | "failed" | "stopped".
    pub status: String,
    /// Launch agent/model the run was started with (carried across stages when a
    /// lane has no override of its own).
    #[serde(default)]
    pub launch_model: Option<String>,
    #[serde(default)]
    pub launch_agent: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
