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

/// A pane's viewport: its size in CSS pixels and how many device pixels it wants per CSS
/// pixel.
///
/// The scale is why this is a type rather than three numbers. Frames used to be captured
/// at one device pixel per CSS pixel and then drawn on a display with two, which upscales
/// every glyph and reads as a pane that is out of focus next to the rest of the app.
/// Capturing at the client's own ratio makes it pixel-exact — but a pane is not allowed to
/// ask for an unbounded image, so the ratio is clamped against the width here, where the
/// rule can be tested, rather than at the call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    width: u32,
    height: u32,
    scale: f64,
}

/// Widest capture, in device pixels. Beyond this the JPEG costs more than the sharpness is
/// worth on a screencast that repaints continuously.
const MAX_CAPTURE_WIDTH: f64 = 2560.0;

/// The most device pixels per CSS pixel a pane can be given.
///
/// Not a taste decision: the browser is launched at this scale
/// (`--force-device-scale-factor`, see [`super::chrome`]) and its compositor surface is
/// what a screencast frame is copied from. Asking for more than the surface holds does not
/// produce more detail — it produces a frame whose size no longer matches what the pane
/// thinks it is clicking on.
pub(super) const MAX_RATIO: f64 = 2.0;
/// Chromium rejects a zero viewport, and a pane narrower than this is not usable anyway.
const MIN_EXTENT: u32 = 200;
const MAX_WIDTH: u32 = 3840;
const MAX_HEIGHT: u32 = 2160;

impl Viewport {
    /// Clamp a pane's request into something Chromium will accept and this process is
    /// willing to encode. `ratio` is the client's `devicePixelRatio`; anything absent,
    /// negative or not finite falls back to 1.
    pub fn new(width: u32, height: u32, ratio: Option<f64>) -> Self {
        let width = width.clamp(MIN_EXTENT, MAX_WIDTH);
        let height = height.clamp(MIN_EXTENT, MAX_HEIGHT);
        let requested = match ratio {
            Some(value) if value.is_finite() && value >= 1.0 => value.min(MAX_RATIO),
            _ => 1.0,
        };
        // A wide pane on a retina display would otherwise ask for a 7680px frame.
        let affordable = MAX_CAPTURE_WIDTH / f64::from(width);
        Self {
            width,
            height,
            scale: requested.min(affordable).max(1.0),
        }
    }

    pub fn width(self) -> u32 {
        self.width
    }

    pub fn height(self) -> u32 {
        self.height
    }

    /// Device pixels per CSS pixel, as applied. Reported back to the pane, which needs it
    /// to turn a click on the frame into a coordinate on the page.
    pub fn scale(self) -> f64 {
        self.scale
    }

    /// How wide a screencast frame of this viewport should be, in pixels.
    ///
    /// This is the only thing that decides frame resolution. The emulated device scale
    /// factor does not: it changes what the *page* believes its pixel ratio is — which is
    /// how a site is persuaded to serve its retina images — while the frame is copied from
    /// a compositor surface sized by the browser's own scale. Capping the copy here is
    /// what keeps a one-to-one display cheap and a retina one exact.
    pub fn capture_width(self) -> u32 {
        (f64::from(self.width) * self.scale).round() as u32
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
