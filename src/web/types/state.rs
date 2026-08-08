//! App state snapshot and shared server state types.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::mcp::NvimSocketRegistry;
use crate::mcp_skills::SkillsRegistry;

use super::super::pty_manager::WebPtyHandle;
use super::super::web_state::WebStateHandle;
use super::events::{EditorEvent, WebEvent};

// ── App state snapshot ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct WebAppState {
    /// True after all configured projects have completed their first session fetch.
    pub startup_ready: bool,
    pub projects: Vec<WebProjectInfo>,
    pub active_project: usize,
    pub panels: WebPanelVisibility,
    pub focused: String,
    /// Optional instance name (derived from tunnel hostname/name).
    /// Used as the page title in the web UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_name: Option<String>,
    /// Active agent backend ("opencode" or "claude-code").
    ///
    /// This is the *CLI* opman was launched with, not a runner name: both claude
    /// engines report "claude-code" here, so it cannot identify a runner. Use
    /// `default_runner` for that.
    pub backend: String,
    /// Runner that owns sessions with no explicit runner of their own. This is
    /// the only correct default for a new session.
    pub default_runner: String,
    /// Runners available for runtime selection in the prompt input.
    pub runners: Vec<String>,
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
    /// Sessions that have encountered an error.
    pub error_sessions: Vec<String>,
    /// Sessions that need user input (pending permission or question).
    pub input_sessions: Vec<String>,
    /// Sessions with unseen activity (completed or errored while not viewed).
    pub unseen_sessions: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct WebSessionInfo {
    pub id: String,
    pub title: String,
    #[serde(rename = "parentID")]
    pub parent_id: String,
    pub directory: String,
    pub time: WebSessionTime,
    /// Runner used for the most recent turn in this session.
    #[serde(default)]
    pub runner: String,
    /// Model, agent, effort and permission mode, as this session's runner reports them.
    ///
    /// Carried on the session row rather than fetched per selection: the client already
    /// re-reads this list on every state change, so switching sessions costs no request
    /// and the composer never renders a stale configuration while one is in flight.
    #[serde(default)]
    pub engine: crate::app::EngineChoices,
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
    /// Shared neovim socket registry, still used by the terminal and MCP tools.
    pub nvim_registry: NvimSocketRegistry,
    /// Running language servers for the file editor's LSP features. Started on
    /// demand per (project root, language); no Neovim session required.
    pub lsp: std::sync::Arc<crate::lsp::LspPool>,
    /// Skills registry for MCP server.
    pub skills_registry: SkillsRegistry,
    /// The MCP server set handed to runners. Swappable, so the settings page can add,
    /// remove, or toggle a server and have it apply without restarting opman.
    pub mcp: crate::mcp_registry::RegistryHandle,
    /// Broadcast sender for skills reload.
    pub reload_tx: broadcast::Sender<()>,
    /// Optional instance name (from tunnel hostname subdomain or tunnel name).
    /// Sent to the frontend as the page title.
    pub instance_name: Option<String>,
    /// Active agent backend name, e.g. "opencode" or "claude-code".
    pub backend: String,
    /// Broadcast channel for editor-specific file-change events.
    /// Consumed by the `/api/editor/events` SSE endpoint.
    pub editor_tx: broadcast::Sender<EditorEvent>,
    /// Process health monitoring handle.
    pub health: crate::process_health::HealthHandle,
    /// Shared secret guarding the loopback-only `/internal/*` API used by the
    /// Kanban and ask MCP servers. Written to `~/.config/opman/internal.json` at startup.
    pub internal_token: String,
    /// Questions raised through the `ask` MCP server and waiting on a human. Held here
    /// rather than in an engine because the asker is a child of the runner, not of any
    /// engine: the request that waits and the reply that answers it only meet in the web
    /// server.
    pub ask_pending: std::sync::Arc<crate::web::ask_pending::AskPending>,
    /// Common runtime runner registry used by session handlers.
    pub runner_registry: std::sync::Arc<crate::runner::RunnerRegistry>,
    /// The live ACP engines. Reconcilable against `acp.json`, so the settings page can add,
    /// edit or remove an agent and have the runner appear or disappear without a restart.
    pub acp: std::sync::Arc<crate::acp_engine::supervisor::AcpSupervisor>,
    /// MCP OAuth logins waiting on a browser. Held here rather than per request because
    /// the flow outlives the request that started it: the settings page gets the
    /// authorize URL back immediately and delivers the callback in a second call.
    pub mcp_logins: std::sync::Arc<crate::web::handlers::LoginSessions>,
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
