//! Native Language Server Protocol client.
//!
//! The editor's hover, diagnostics, go-to-definition and format used to be
//! proxied through a Neovim instance, which meant they only worked when a
//! Neovim session happened to be open — in practice, never. This module talks
//! to language servers directly: it picks the server from the file's extension,
//! finds the project root the server should be rooted at, starts it on first
//! use, keeps it warm for the whole workspace, and shuts it down when it goes
//! idle.
//!
//! Layering, outermost first:
//!
//! * [`api`] — the four operations, in the JSON shapes the web editor expects.
//! * [`pool`] — which servers are running, keyed by (root, language).
//! * [`server`] — one process: spawn, `initialize` handshake, capabilities.
//! * [`docs`] / [`diags`] — what the server has been told, and what it has said.
//! * [`peer`] / [`framing`] — JSON-RPC over `Content-Length`-framed stdio.
//! * [`detect`] / [`convert`] — the lookup tables and the unit conversions.

pub mod api;
pub mod api_edit;
pub mod completion;
pub mod convert;
pub mod detect;
pub mod diags;
pub mod docs;
pub mod framing;
pub mod notify;
pub mod peer;
pub mod pool;
pub mod reaper;
pub mod server;

pub use pool::{LspPool, ServerKey};

#[cfg(test)]
#[path = "live_tests.rs"]
mod live_tests;
