//! Cloudflare Tunnel integration.
//!
//! Spawns `cloudflared` as a child process to expose the web UI via a
//! Cloudflare tunnel.  Three modes are supported:
//!
//! - **Local-managed tunnel** (`--tunnel-hostname <HOSTNAME>`): Fully automatic.
//!   On first run, opens a browser for Cloudflare login, creates the tunnel,
//!   DNS record, and ingress config. On subsequent runs, reuses existing
//!   credentials. This is the recommended approach.
//!
//! - **Named (remote-managed) tunnel** (`--tunnel-token <TOKEN>`): Uses a
//!   pre-configured tunnel from the Cloudflare dashboard
//!   (`cloudflared tunnel run --token <token>`).
//!
//! - **Quick tunnel** (`--tunnel`): Ephemeral `trycloudflare.com` URL
//!   (`cloudflared tunnel --url http://localhost:<port>`).
//!
//! The tunnel process is killed when the returned `TunnelHandle` is dropped.

pub(crate) mod helpers;
pub(crate) mod local_managed;
pub(crate) mod local_managed_setup;
mod named;
mod quick;
mod tests;
mod types;

pub use types::{spawn_tunnel, TunnelHandle, TunnelMode, TunnelOptions};
