//! Kanban board domain types (board, lanes, transition graph, tasks, notes,
//! attachments) plus the request/response shapes used by the HTTP handlers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One column in the board's state-graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    pub id: String,
    pub name: String,
    pub color: String,
    /// Optional WIP limit (None = unlimited).
    #[serde(default)]
    pub wip: Option<u32>,
    /// Agents stop here for human sign-off (e.g. "In Review").
    #[serde(default)]
    pub terminal: bool,
    /// Default agent auto-selected when launching a task in this lane.
    #[serde(default)]
    pub agent: Option<String>,
    /// Optional default model for this lane (overridable at launch).
    #[serde(default)]
    pub model: Option<String>,
    /// Per-stage prompt used by pipeline-mode launches. Each stage runs in its
    /// own session seeded with this prompt plus the previous stage's output.
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Adjacency list of allowed transitions: `lane_id -> [allowed target lane_ids]`.
pub type Transitions = HashMap<String, Vec<String>>;

/// A per-project board: ordered lanes + the transition graph between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub transitions: Transitions,
}

impl Board {
    /// Whether a task may move from `from` lane to `to` lane.
    /// Same-lane moves (reordering) are always allowed.
    pub fn transition_allowed(&self, from: &str, to: &str) -> bool {
        if from == to {
            return true;
        }
        self.transitions
            .get(from)
            .map(|targets| targets.iter().any(|t| t == to))
            .unwrap_or(false)
    }

    /// The board's terminal review lane id (where agents stop), if any.
    pub fn terminal_lane_id(&self) -> Option<&str> {
        self.lanes
            .iter()
            .find(|l| l.terminal)
            .map(|l| l.id.as_str())
    }

    /// Lane lookup by id.
    pub fn lane(&self, id: &str) -> Option<&Lane> {
        self.lanes.iter().find(|l| l.id == id)
    }
}

/// A kanban task / card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub board_id: String,
    pub lane_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub order_index: f64,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub launch_model: Option<String>,
    #[serde(default)]
    pub launch_agent: Option<String>,
    #[serde(default = "default_run_state")]
    pub run_state: String,
    pub created_at: String,
    pub updated_at: String,
}

fn default_priority() -> String {
    "normal".to_string()
}
fn default_run_state() -> String {
    "idle".to_string()
}

/// An uploaded attachment (image / video / file). The binary lives on disk;
/// only metadata is stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    #[serde(skip)]
    pub task_id: String,
    pub filename: String,
    pub mime: String,
    pub kind: String,
    pub size_bytes: i64,
    pub created_at: String,
    /// Public URL to fetch the asset. Computed on read.
    #[serde(default)]
    pub url: String,
}

/// An append-only progress note (agent or user) on a task's activity timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanNote {
    pub id: String,
    pub author: String,
    pub body: String,
    #[serde(default)]
    pub lane_from: Option<String>,
    #[serde(default)]
    pub lane_to: Option<String>,
    pub created_at: String,
}

// ── Responses ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BoardResponse {
    pub board: Board,
    pub tasks: Vec<Task>,
    /// Active/finished pipeline runs for the board's tasks. Lets the UI tag every
    /// stage session to its own lane (a task only carries its *current* session).
    #[serde(default)]
    pub pipelines: Vec<PipelineRun>,
}

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
    /// "running" | "done" | "failed".
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

#[derive(Debug, Clone, Serialize)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub task: Task,
    pub notes: Vec<KanbanNote>,
    pub attachments: Vec<Attachment>,
}

// ── Requests ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub board_id: String,
    pub lane_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_priority")]
    pub priority: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub lane_id: Option<String>,
    #[serde(default)]
    pub order_index: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct BoardConfigRequest {
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub transitions: Transitions,
}

#[derive(Debug, Deserialize)]
pub struct LaunchTaskRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    /// Launch mode: "single" (one session walks the whole board, default) or
    /// "pipeline" (each lane runs in its own session, chained by output).
    #[serde(default)]
    pub mode: Option<String>,
}

/// User-authored note posted from the board UI (distinct from agent notes).
#[derive(Debug, Deserialize)]
pub struct UserNoteRequest {
    pub body: String,
}

/// Internal (MCP-facing) request to move a task's lane.
#[derive(Debug, Deserialize)]
pub struct InternalStatusRequest {
    pub lane: String,
    #[serde(default)]
    pub run_state: Option<String>,
}

/// Internal (MCP-facing) request to append a note.
#[derive(Debug, Deserialize)]
pub struct InternalNoteRequest {
    pub body: String,
    #[serde(default)]
    pub lane_from: Option<String>,
    #[serde(default)]
    pub lane_to: Option<String>,
}

// ── Defaults ────────────────────────────────────────────────────────

/// Classify a mime type into image / video / file.
pub fn kind_from_mime(mime: &str) -> &'static str {
    if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else {
        "file"
    }
}

/// Build the default board for a freshly-opened project.
pub fn default_board(id: String, project_path: String) -> Board {
    let lane = |id: &str, name: &str, color: &str, agent: Option<&str>, terminal: bool| Lane {
        id: id.to_string(),
        name: name.to_string(),
        color: color.to_string(),
        wip: None,
        terminal,
        agent: agent.map(|a| a.to_string()),
        model: None,
        prompt: None,
    };
    let lanes = vec![
        lane("lane_todo", "Todo", "#8b95a7", None, false),
        lane("lane_planning", "Planning", "#7aa2f7", Some("plan"), false),
        lane("lane_implementing", "Implementing", "#bb9af7", Some("build"), false),
        lane("lane_validating", "Validating", "#e0af68", Some("build"), false),
        lane("lane_codereview", "Code Review", "#f7768e", Some("code-reviewer"), false),
        lane("lane_inreview", "In Review", "#73daca", None, true),
        lane("lane_done", "Done", "#9ece6a", None, false),
    ];
    // Forward edge to the next lane + a backward edge to the previous lane (rework).
    let mut transitions: Transitions = HashMap::new();
    let ids: Vec<String> = lanes.iter().map(|l| l.id.clone()).collect();
    for (i, id) in ids.iter().enumerate() {
        let mut targets = Vec::new();
        if i + 1 < ids.len() {
            targets.push(ids[i + 1].clone());
        }
        if i > 0 {
            targets.push(ids[i - 1].clone());
        }
        transitions.insert(id.clone(), targets);
    }
    Board {
        id,
        name: "Board".to_string(),
        project_path,
        lanes,
        transitions,
    }
}
