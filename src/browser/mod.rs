//! Headless browser panes.
//!
//! A browser widget is a real Chromium page driven over the DevTools protocol. The pane
//! shows it as an iframe when the site allows framing and as a live screencast when it
//! does not; the agent acts on the *same* page either way, through
//! [`crate::mcp_browser`].
//!
//! The reason this is not just "give the model the HTML": a modern page is 200–800 KB of
//! markup, most of it framework noise, and re-reading it after every click is what makes
//! browser agents expensive. [`tab::Tab::snapshot`] instead returns an indented outline of
//! only the actionable and structural nodes, each tagged `[ref=eN]`, which the model
//! clicks by. That is typically 1–2 KB, and it stays constant as pages grow.

mod cdp;
mod chrome;
mod input;
mod pool;
mod screencast;
mod tab;
mod types;

#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;

pub use input::MouseKind;
pub use pool::{normalize_url, pane_id_for_project, BrowserPool, Opened, Pane, BLANK};
pub use screencast::{Screencast, Viewer};
pub use tab::Tab;
pub use types::{PageSnapshot, PageText, PaneInfo, RenderMode, SnapshotOptions};
