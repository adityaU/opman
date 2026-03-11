//! App state snapshot and shared server state types.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::mcp::NvimSocketRegistry;

use super::super::pty_manager::WebPtyHandle;
use super::super::web_state::WebStateHandle;
use super::events::WebEvent;

// ── App state snapshot ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct WebAppState {
    pub projects: Vec<WebProjectInfo>,
    pub active_project: usize,
    pub panels: WebPanelVisibility,
    pub focused: String,
    /// Optional instance name (derived from tunnel hostname/name).
    /// Used as the page title in the web UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct WebProjectInfo {
    pub name: String,
    pub path: String,
    pub index: usize,
    pub active_session: Option<String>,
    pub sessions: Vec<WebSessionInfo>,
    pub git_branch: String,
    pub busy_sessions: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct WebSessionInfo {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub directory: String,
    pub time: WebSessionTime,
}

#[derive(Serialize, Clone)]
pub struct WebSessionTime {
    pub created: u64,
    pub updated: u64,
}

#[derive(Serialize, Clone)]
pub struct WebPanelVisibility {
    pub sidebar: bool,
    pub terminal_pane: bool,
    pub neovim_pane: bool,
    pub integrated_terminal: bool,
    pub git_panel: bool,
}

/// Shared state available to all Axum handlers via `State<ServerState>`.
#[derive(Clone)]
pub struct ServerState {
    /// Independent web state manager (talks directly to opencode API).
    pub web_state: WebStateHandle,
    /// JWT signing secret (random per run).
    pub jwt_secret: Vec<u8>,
    /// Expected username (empty = no auth required).
    pub username: String,
    /// Expected password.
    pub password: String,
    /// Broadcast channel for app events (state changes, busy/idle, etc.).
    pub event_tx: broadcast::Sender<WebEvent>,
    /// Broadcast channel for raw upstream opencode SSE events.
    /// Each value is the raw JSON string from the upstream `/event` stream
    /// (already extracted from the `data:` SSE field).  The web
    /// `session_events_stream` subscribes here instead of opening a separate
    /// upstream connection (the opencode server may limit concurrent SSE
    /// consumers per project).
    pub raw_sse_tx: broadcast::Sender<String>,
    /// Handle to the web PTY manager (independent from TUI PTYs).
    pub pty_mgr: WebPtyHandle,
    /// Shared HTTP client for proxying requests to the opencode server.
    /// Reuses TCP connections across requests (connection pooling).
    pub http_client: reqwest::Client,
    /// Shared neovim socket registry for LSP-backed editor features.
    pub nvim_registry: NvimSocketRegistry,
    /// Optional instance name (from tunnel hostname subdomain or tunnel name).
    /// Sent to the frontend as the page title.
    pub instance_name: Option<String>,
}
