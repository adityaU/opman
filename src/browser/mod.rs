//! Browser panes.
//!
//! A browser widget is a real Chromium page driven over the DevTools protocol — headed,
//! on a virtual display, because headless is blocked by a lot of the web. The pane
//! shows it as an iframe when the site allows framing and as a live screencast when it
//! does not; the agent acts on the *same* page either way, through
//! [`crate::mcp_browser`].
//!
//! The reason this is not just "give the model the HTML": a modern page is 200–800 KB of
//! markup, most of it framework noise, and re-reading it after every click is what makes
//! browser agents expensive. [`tab::Tab::snapshot`] instead returns an indented outline of
//! only the actionable and structural nodes, each tagged `[ref=eN]`, which the model
//! clicks by. That is typically 1–2 KB, and it stays constant as pages grow.

mod banner;
mod binary;
mod candidates;
mod cdp;
mod chrome;
mod display;
mod display_device;
mod engine;
mod flags;
mod input;
mod install;
mod mode;
mod pane;
mod pool;
mod profile;
mod relay;
mod relay_pipe;
mod screencast;
mod tab;
mod types;
mod window;

#[cfg(test)]
pub(crate) mod fake_device;

#[cfg(test)]
#[path = "link_tests.rs"]
mod link_tests;

#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;

#[cfg(test)]
#[path = "live_render_tests.rs"]
mod live_render_tests;

#[cfg(test)]
#[path = "live_device_tests.rs"]
mod live_device_tests;

pub use binary::{BrowserInstallGuide, BrowserUnavailable};
pub use engine::DeviceLink;
#[cfg(test)]
pub use engine::{LinkId, LinkState};
pub use mode::BrowserMode;
// The home side of a device link. Called by the remote-server supervisor, which keeps one
// relay per configured remote while this opman runs with `--device-browser`.
#[allow(unused_imports)]
pub use relay::{run_device_link, run_device_link_loop, RelayEnd, RelayError};
pub use input::MouseKind;
pub use pane::Pane;
pub use pool::{normalize_url, pane_id_for_project, BrowserPool, Opened, BLANK};
pub use screencast::{Screencast, Viewer};
pub use tab::Tab;
pub use types::{PageSnapshot, PageText, PaneInfo, RenderMode, SnapshotOptions, Viewport};
