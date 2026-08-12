//! Authenticated WebSocket transport for one browser Neovim viewport.
//!
//! Neovim's UI channel has one viewport when `ext_multigrid` is disabled. A
//! later browser connection therefore supersedes the earlier one; sharing a
//! channel would make the tabs race over `try_resize` and corrupt the view.

mod egress;
mod handler;
mod ingress;

pub(crate) use handler::websocket_handler;
