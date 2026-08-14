//! Wire types shared by the HTTP handlers, the MCP server, and the pane.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// How a pane is showing the page. Server-decided, because the answer lives in response
/// headers the browser tab is not allowed to read back from an iframe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    /// The site permits framing: the pane shows a real iframe and pays no streaming cost.
    Iframe,
    /// Framing is refused, or the user pinned mirroring on: the pane draws CDP frames.
    Screencast,
}

impl RenderMode {
    /// `X-Frame-Options` and CSP `frame-ancestors` are the only two headers that can
    /// refuse framing. Anything else — including no headers at all — means iframe is safe
    /// to try, and the pane can still be flipped to screencast by hand.
    pub fn from_headers(x_frame_options: Option<&str>, csp: Option<&str>) -> Self {
        let refused_by_xfo = x_frame_options.is_some_and(|value| {
            let value = value.trim().to_ascii_lowercase();
            value.starts_with("deny") || value.starts_with("sameorigin")
        });
        let refused_by_csp = csp.is_some_and(|value| {
            value
                .to_ascii_lowercase()
                .split(';')
                .filter_map(|directive| directive.trim().strip_prefix("frame-ancestors"))
                .any(|rest| !rest.contains('*'))
        });
        if refused_by_xfo || refused_by_csp {
            Self::Screencast
        } else {
            Self::Iframe
        }
    }
}

/// One page's compact state, as both the pane and the LLM see it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub scroll_y: i64,
    #[serde(default)]
    pub scroll_height: i64,
    #[serde(default)]
    pub viewport_height: i64,
    #[serde(default)]
    pub ref_count: usize,
    #[serde(default)]
    pub truncated: bool,
    /// The indented `[ref=eN]` outline — never HTML.
    pub outline: String,
}

/// Main-content text, for reading rather than acting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageText {
    pub url: String,
    pub title: String,
    pub truncated: bool,
    pub text: String,
}

/// Knobs the caller may turn to trade detail against tokens.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SnapshotOptions {
    #[serde(default = "default_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    #[serde(default = "default_max_text_len")]
    pub max_text_len: usize,
    /// Restrict to what is on screen. Off by default: an LLM asking "what is on this
    /// page" means the page, not the fold.
    #[serde(default)]
    pub viewport_only: bool,
}

const fn default_max_nodes() -> usize {
    400
}
const fn default_max_chars() -> usize {
    12_000
}
const fn default_max_text_len() -> usize {
    120
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            max_nodes: default_max_nodes(),
            max_chars: default_max_chars(),
            max_text_len: default_max_text_len(),
            viewport_only: false,
        }
    }
}

/// What a pane looks like from the outside — enough for `browser_list_panes` to let an
/// LLM pick a target without opening anything.
#[derive(Clone, Debug, Serialize)]
pub struct PaneInfo {
    pub pane_id: Arc<str>,
    /// The project this browser belongs to — browsers are per project.
    pub project: Arc<str>,
    pub url: String,
    pub title: String,
    pub mode: RenderMode,
}

/// Where a click, key, or scroll came from. Pane input and tool input take the same path
/// into CDP; only the origin differs, and only for logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    Human,
    Agent,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod types_tests;
