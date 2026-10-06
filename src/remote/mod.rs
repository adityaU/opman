//! Remote opman servers, reached through this ("home") server.
//!
//! The browser only ever talks to home. Home keeps the remote list and its credentials in
//! `~/.config/opman/servers.json` (0600), logs in to each remote with its
//! `POST /api/auth/login`, and the web layer (`crate::web::remote`) forwards
//! `/remote/{id}/api/*` there with the remote's bearer. See
//! `docs/multi-server-device-browser.md` for the full contract.
//!
//! # API for other subsystems
//!
//! [`Registry`] is held in `ServerState::remote` and is cheap to clone:
//!
//! ```ignore
//! // The current list, without credentials.
//! async fn servers(&self) -> Vec<ServerSnapshot>;
//! // An authenticated WebSocket to `{remote}{path}`; a 401 handshake re-logs in once.
//! async fn connect_ws(&self, id: &ServerId, path: &str) -> Result<RemoteWs, RemoteError>;
//! // Fires (generation counter) on every add / edit / remove.
//! fn subscribe(&self) -> tokio::sync::watch::Receiver<u64>;
//! ```
//!
//! A per-server supervisor (e.g. the device-browser relay) loops on
//! `subscribe().changed()`, diffs `servers()` against the tasks it runs, and starts or
//! stops one task per [`ServerId`]. An edit replaces the snapshot (same id, new url or
//! name), so compare whole snapshots, not just ids, to know when to reconnect.

mod headers;
mod registry;
mod session;
mod store;
mod types;
mod ws;

pub use headers::{downstream_response, upstream_query, upstream_request};
pub use registry::{NewServer, Registry, ServerPatch};
pub use session::{Outbound, OutboundBody, Remote};
#[cfg(test)]
pub use store::ServerStore;
pub use types::{RemoteError, ServerId, ServerSnapshot, ServerStatus, HOME_ID};
pub use ws::RemoteWs;
