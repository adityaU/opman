use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

// ─── Shared neovim socket registry ──────────────────────────────────────────

/// Shared registry of neovim socket paths, keyed by (project_idx, session_id).
/// Updated by the main loop when neovim PTYs are spawned; read by socket server
/// tasks to handle nvim operations directly without round-tripping through the
/// main event loop.
pub type NvimSocketRegistry = Arc<tokio::sync::RwLock<HashMap<(usize, String), PathBuf>>>;

/// Create a new empty neovim socket registry.
pub fn new_nvim_socket_registry() -> NvimSocketRegistry {
    Arc::new(tokio::sync::RwLock::new(HashMap::new()))
}

// ─── Internal socket protocol ───────────────────────────────────────────────

/// A single edit operation within a multi-edit batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOp {
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub new_text: String,
}

/// Request sent over Unix socket from MCP bridge → manager.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SocketRequest {
    pub op: String, // "read" | "run" | "list" | "new" | "close" | "rename"
    // + neovim ops: "nvim_open" | "nvim_read" | "nvim_command" | "nvim_buffers" | "nvim_info"
    //   "nvim_diagnostics" | "nvim_definition" | "nvim_references"
    //   "nvim_hover" | "nvim_symbols" | "nvim_code_actions"
    //   "nvim_eval" | "nvim_grep" | "nvim_diff" | "nvim_write"
    //   "nvim_edit_and_save" | "nvim_undo" | "nvim_rename" | "nvim_format" | "nvim_signature"
    /// Session ID for routing to the correct per-session resources.
    /// Set by MCP bridges from OPENCODE_SESSION_ID env var.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tab: Option<usize>, // tab index (0-based)
    #[serde(default)]
    pub command: Option<String>, // for "run" op and "nvim_command" / "nvim_eval"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>, // for "new" and "rename" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<bool>, // for "run" op: wait for output to settle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_n: Option<usize>, // for "read" op: return only last N lines
    // ── Neovim-specific fields ──────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>, // for "nvim_open" / "nvim_grep" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>, // for "nvim_open" / "nvim_read" / LSP position ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>, // for "nvim_read" op (end of range)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<i64>, // column for LSP position ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>, // for "nvim_symbols" / "nvim_grep" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buf_only: Option<bool>, // for "nvim_diagnostics": current buffer only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<bool>, // for "nvim_symbols": workspace vs document
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<bool>, // for "nvim_write": write all buffers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glob: Option<String>, // for "nvim_grep": file glob pattern
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_text: Option<String>, // for "nvim_edit_and_save": replacement text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>, // for "nvim_undo": undo count (negative = redo)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>, // for "nvim_rename": new symbol name
    // ── Multi-edit batch ────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<EditOp>>, // for "nvim_edit_and_save": batch of edits
}

/// Response sent over Unix socket from manager → MCP bridge.
#[derive(Debug, Serialize, Deserialize)]
pub struct SocketResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabInfo {
    pub index: usize,
    pub active: bool,
    pub name: String,
}

impl SocketResponse {
    pub fn ok_text(output: String) -> Self {
        Self {
            ok: true,
            output: Some(output),
            tabs: None,
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_tabs(tabs: Vec<TabInfo>) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: Some(tabs),
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_tab_created(tab_index: usize) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: Some(tab_index),
            command_state: None,
        }
    }
    pub fn ok_empty() -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn err(msg: String) -> Self {
        Self {
            ok: false,
            output: None,
            tabs: None,
            error: Some(msg),
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_status(state: String) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: None,
            command_state: Some(state),
        }
    }
}

/// A pending socket request paired with a oneshot channel for the response.
pub struct PendingSocketRequest {
    pub request: SocketRequest,
    pub reply_tx: tokio::sync::oneshot::Sender<SocketResponse>,
}

// ─── Socket path helper ─────────────────────────────────────────────────────

/// Compute the Unix socket path for a given project path.
/// Format: /tmp/opman-{hash}.sock
pub fn socket_path_for_project(project_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    project_path.hash(&mut hasher);
    let hash = hasher.finish();
    PathBuf::from(format!("/tmp/opman-{:x}.sock", hash))
}

// ─── Cleanup: remove socket files on shutdown ───────────────────────────────

pub fn cleanup_socket(project_path: &Path) {
    let sock = socket_path_for_project(project_path);
    let _ = std::fs::remove_file(&sock);
}
