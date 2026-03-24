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
    /// A session encountered an error.
    SessionError {
        session_id: String,
        message: String,
    },
    /// A session needs user input (permission or question pending).
    SessionInputNeeded {
        session_id: String,
    },
    /// A session no longer needs user input.
    SessionInputCleared {
        session_id: String,
    },
    /// A session has unseen activity (idle or error while not viewed).
    SessionUnseen {
        session_id: String,
        count: usize,
    },
    /// A session's unseen state was cleared (user viewed it).
    SessionSeen {
        session_id: String,
    },
    StatsUpdated(WebSessionStats),
    ThemeChanged(WebThemePair),
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
    /// Mission state changed (created, updated, state transition).
    MissionUpdated {
        mission: serde_json::Value,
    },
    /// Routine configuration or run state changed.
    RoutineUpdated,
    /// Toast notification from the TUI (status bar messages).
    Toast {
        message: String,
        /// One of: "info", "success", "warning", "error".
        level: String,
    },
    /// Initial value — never sent to clients.
    #[allow(dead_code)]
    Noop,
}

// ── Editor events (separate SSE channel) ────────────────────────────

/// Events pushed to web clients via the dedicated `/api/editor/events` SSE stream.
/// Kept separate from `WebEvent` so editor-specific traffic doesn't mix with
/// app-level broadcasts and the opencode proxy SSE.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum EditorEvent {
    /// A file on disk was modified — either via the web save endpoint or
    /// by an AI agent (upstream `file.edited`).
    FileChanged {
        /// Relative path within the project directory.
        path: String,
        /// Origin of the change: `"web_save"` or `"ai_edit"`.
        source: String,
    },
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

/// Theme preview: name + resolved colors for both appearances.
#[derive(Serialize, Clone)]
pub struct ThemePreview {
    pub name: String,
    pub dark: WebThemeColors,
    pub light: WebThemeColors,
}

/// Both dark and light variants of the active theme.
/// Sent via bootstrap, `/api/theme`, SSE `theme_changed`, and `/api/theme/switch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebThemePair {
    pub dark: WebThemeColors,
    pub light: WebThemeColors,
}

impl WebThemePair {
    /// Build from the currently active theme name by loading both variants.
    pub fn from_active_theme() -> Self {
        let dark = WebThemeColors::from_theme(&crate::theme::load_theme_with_mode("dark"));
        let light = WebThemeColors::from_theme(&crate::theme::load_theme_with_mode("light"));
        Self { dark, light }
    }
}

/// Request to switch the active theme.
#[derive(Deserialize)]
pub struct SwitchThemeRequest {
    pub name: String,
}
