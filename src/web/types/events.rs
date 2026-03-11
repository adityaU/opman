//! Broadcast events and theme types for the web API.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::activity::ActivityEventPayload;
use super::presence::PresenceSnapshot;
use super::sessions::WebSessionStats;
use super::watchers::WatcherStatusEvent;

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
    /// Watcher status changed (created, deleted, countdown, triggered).
    WatcherStatusChanged(WatcherStatusEvent),
    /// MCP: AI agent opened a file in the editor.
    McpEditorOpen {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
    },
    /// MCP: AI agent navigated to a line in the editor.
    #[allow(dead_code)]
    McpEditorNavigate {
        line: u32,
    },
    /// MCP: AI agent focused a terminal tab.
    McpTerminalFocus {
        id: String,
    },
    /// MCP: AI agent activity indicator (tool being invoked).
    McpAgentActivity {
        tool: String,
        active: bool,
    },
    /// Session activity event (file edit, tool call, terminal command, permission request).
    ActivityEvent(ActivityEventPayload),
    /// Presence changed — a client connected/disconnected or focused a session.
    PresenceChanged(PresenceSnapshot),
    /// Initial value — never sent to clients.
    #[allow(dead_code)]
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
