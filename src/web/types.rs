//! Serializable types for the web API.
//!
//! These mirror the internal App/Session/PTY types but are decoupled for
//! independent evolution and to avoid leaking internal details.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::pty_manager::WebPtyHandle;
use super::web_state::WebStateHandle;

// ── Broadcast events from web state → web clients ───────────────────

/// Events pushed to connected web clients via SSE.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum WebEvent {
    StateChanged,
    SessionBusy {
        session_id: String,
    },
    SessionIdle {
        session_id: String,
    },
    StatsUpdated(WebSessionStats),
    ThemeChanged(WebThemeColors),
    /// Initial value — never sent to clients.
    Noop,
}

// ── Theme colors (hex strings for the web frontend) ─────────────────

/// Serializable theme colors — 15 hex string fields matching the TUI's `ThemeColors` struct.
/// Sent to web clients via `/api/theme` and SSE `theme_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebThemeColors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub background: String,
    pub background_panel: String,
    pub background_element: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
    pub border_active: String,
    pub border_subtle: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub info: String,
}

impl WebThemeColors {
    /// Convert a TUI `ThemeColors` into serializable hex strings.
    pub fn from_theme(theme: &crate::theme::ThemeColors) -> Self {
        Self {
            primary: color_to_hex(theme.primary),
            secondary: color_to_hex(theme.secondary),
            accent: color_to_hex(theme.accent),
            background: color_to_hex(theme.background),
            background_panel: color_to_hex(theme.background_panel),
            background_element: color_to_hex(theme.background_element),
            text: color_to_hex(theme.text),
            text_muted: color_to_hex(theme.text_muted),
            border: color_to_hex(theme.border),
            border_active: color_to_hex(theme.border_active),
            border_subtle: color_to_hex(theme.border_subtle),
            error: color_to_hex(theme.error),
            warning: color_to_hex(theme.warning),
            success: color_to_hex(theme.success),
            info: color_to_hex(theme.info),
        }
    }
}

/// Convert a ratatui `Color` to a CSS hex string (e.g. `#fab283`).
fn color_to_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        _ => "#808080".to_string(), // Fallback for non-RGB colors
    }
}

// ── App state snapshot ──────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct WebAppState {
    pub projects: Vec<WebProjectInfo>,
    pub active_project: usize,
    pub panels: WebPanelVisibility,
    pub focused: String,
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

#[derive(Debug, Serialize, Clone, Default)]
pub struct WebSessionStats {
    pub cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

// ── Request body types ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

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

#[derive(Deserialize)]
pub struct TogglePanelRequest {
    pub panel: String,
}

#[derive(Deserialize)]
pub struct FocusPanelRequest {
    pub panel: String,
}

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

#[derive(Deserialize)]
pub struct SseTokenQuery {
    pub token: Option<String>,
    /// PTY ID for terminal stream
    pub id: Option<String>,
}

// ── Proxy request types (opencode API proxy) ────────────────────────

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

/// Theme preview: name + resolved colors for all 15 fields.
#[derive(Serialize, Clone)]
pub struct ThemePreview {
    pub name: String,
    pub colors: WebThemeColors,
}

/// Request to switch the active theme.
#[derive(Deserialize)]
pub struct SwitchThemeRequest {
    pub name: String,
}

/// SSE query params for session event stream (proxied from opencode).
#[derive(Deserialize)]
pub struct SessionSseQuery {
    pub token: Option<String>,
    pub project_dir: Option<String>,
}

/// Pagination query params for message fetching.
#[derive(Deserialize)]
pub struct MessagePaginationQuery {
    /// Maximum number of messages to return (default: all).
    pub limit: Option<usize>,
    /// Number of messages to skip from the start (default: 0).
    pub offset: Option<usize>,
    /// Fetch the last N messages (convenience for initial load).
    /// When set, overrides `offset` to `max(0, total - tail)`.
    pub tail: Option<usize>,
}

// ── Shared Axum state ───────────────────────────────────────────────

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
    /// Handle to the web PTY manager (independent from TUI PTYs).
    pub pty_mgr: WebPtyHandle,
}
