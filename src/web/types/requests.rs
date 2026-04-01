//! Request and response body types for the web API.

use serde::{Deserialize, Serialize};

// ── Auth ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

// ── Project management ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SwitchProjectRequest {
    pub index: usize,
}

#[derive(Deserialize)]
pub struct SelectSessionRequest {
    pub project_idx: usize,
    pub session_id: String,
}

#[derive(Deserialize)]
pub struct NewSessionRequest {
    pub project_idx: usize,
}

/// Response from creating a new session.
#[derive(Serialize)]
pub struct NewSessionResponse {
    pub session_id: String,
}

/// Request to add a new project.
#[derive(Deserialize)]
pub struct AddProjectRequest {
    /// Absolute path to the project directory.
    pub path: String,
    /// Optional display name. If not provided, the directory name is used.
    #[serde(default)]
    pub name: Option<String>,
}

/// Response after successfully adding a project.
#[derive(Serialize)]
pub struct AddProjectResponse {
    /// The index of the newly added project.
    pub index: usize,
    /// The resolved project name.
    pub name: String,
}

/// Request to browse directories (for the add-project picker).
#[derive(Deserialize)]
pub struct BrowseDirsRequest {
    /// Absolute path to list. If empty, defaults to user home.
    #[serde(default)]
    pub path: String,
}

/// A single directory entry in the dir browser.
#[derive(Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    /// Whether this path is already added as a project.
    pub is_project: bool,
}

/// Response for directory browsing.
#[derive(Serialize)]
pub struct BrowseDirsResponse {
    /// The resolved absolute path being listed.
    pub path: String,
    /// Parent directory path (empty if at root).
    pub parent: String,
    /// Child directories.
    pub entries: Vec<DirEntry>,
}

/// Response for the home directory endpoint.
#[derive(Serialize)]
pub struct HomeDirResponse {
    pub path: String,
}

/// Request to remove a project.
#[derive(Deserialize)]
pub struct RemoveProjectRequest {
    pub index: usize,
}

// ── Panel management ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TogglePanelRequest {
    pub panel: String,
}

#[derive(Deserialize)]
pub struct FocusPanelRequest {
    pub panel: String,
}

// ── PTY management ──────────────────────────────────────────────────

/// Request to spawn a web PTY.
#[derive(Deserialize)]
pub struct SpawnPtyRequest {
    /// PTY type: "shell", "neovim", "git", or "opencode"
    pub kind: String,
    /// Unique ID for this PTY instance (client-generated)
    pub id: String,
    pub rows: Option<u16>,
    pub cols: Option<u16>,
    /// Optional session ID (only used for "opencode" kind)
    pub session_id: Option<String>,
}

/// Request to write to a web PTY.
#[derive(Deserialize)]
pub struct PtyWriteRequest {
    /// PTY ID
    pub id: String,
    /// Base64-encoded bytes to write to the PTY.
    pub data: String,
}

/// Request to resize a web PTY.
#[derive(Deserialize)]
pub struct PtyResizeRequest {
    /// PTY ID
    pub id: String,
    pub rows: u16,
    pub cols: u16,
}

/// Request to kill a web PTY.
#[derive(Deserialize)]
pub struct PtyKillRequest {
    /// PTY ID
    pub id: String,
}

// ── SSE / Proxy ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SseTokenQuery {
    pub token: Option<String>,
    /// PTY ID for terminal stream
    pub id: Option<String>,
}

/// Model reference for overriding the default model on a per-message basis.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ModelRef {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

/// Request to send a message to a session.
#[derive(Deserialize, Serialize)]
pub struct SendMessageRequest {
    pub parts: Vec<serde_json::Value>,
    /// Optional model override — sent through to the upstream opencode API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// Optional agent name — sent through to the upstream opencode API per-message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

/// Request to execute a slash command on a session.
#[derive(Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    #[serde(default)]
    pub arguments: String,
    pub model: Option<String>,
}

/// Request to reply to a permission request.
#[derive(Deserialize)]
pub struct PermissionReplyRequest {
    /// "once", "always", or "reject"
    pub reply: String,
}

/// Request to reply to a question.
#[derive(Deserialize)]
pub struct QuestionReplyRequest {
    pub answers: Vec<Vec<String>>,
}

/// Request to rename a session.
#[derive(Deserialize)]
pub struct RenameSessionRequest {
    pub title: String,
}

/// Request for A2UI callback (button click / form submission).
#[derive(Deserialize)]
pub struct A2uiCallbackRequest {
    /// The callback_id from the A2UI block that was interacted with.
    pub callback_id: String,
    /// Payload: form field values, or null for button clicks.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// SSE query params for session event stream (proxied from opencode).
#[derive(Deserialize)]
pub struct SessionSseQuery {
    pub token: Option<String>,
    #[allow(dead_code)]
    pub project_dir: Option<String>,
}
