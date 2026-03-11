//! Core types, options, and entry point for Cloudflare Tunnel integration.

use std::path::PathBuf;
use tokio::process::Child;
use tracing::{error, info, debug};

use super::local_managed::spawn_local_managed;
use super::named::spawn_named;
use super::quick::spawn_quick;

// ── Types ───────────────────────────────────────────────────────────

/// Which tunnel mode to use.
#[derive(Debug, Clone)]
pub enum TunnelMode {
    /// Locally-managed tunnel — automatic setup via `cloudflared tunnel login`,
    /// `create`, `route dns`, then `run` with a generated config file.
    LocalManaged {
        /// Full external hostname, e.g. "opman.example.com".
        hostname: String,
        /// Tunnel name (default "opman"). Must be unique in the CF account.
        tunnel_name: String,
    },
    /// Remote-managed tunnel — `cloudflared tunnel run --token <token>`.
    Named { token: String },
    /// Quick (ephemeral) tunnel — `cloudflared tunnel --url http://localhost:<port>`.
    Quick,
}

/// Returned by [`spawn_tunnel`]. Killing the child on drop.
pub struct TunnelHandle {
    child: Option<Child>,
    /// Temporary config file created for local-managed tunnels.
    /// Cleaned up on drop.
    _config_file: Option<PathBuf>,
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
            info!("Cloudflare tunnel process killed");
        }
        if let Some(ref path) = self._config_file {
            let _ = std::fs::remove_file(path);
            debug!("Removed temporary tunnel config: {}", path.display());
        }
    }
}

// ── Data directory ──────────────────────────────────────────────────

/// `~/.config/opman/tunnel/` — persistent storage for cert.pem and tunnel.json.
pub(crate) fn tunnel_data_dir() -> anyhow::Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine config directory"))?
        .join("opman")
        .join("tunnel");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ── Extra cloudflared options ───────────────────────────────────────

/// Extra cloudflared options passed through from CLI flags.
#[derive(Debug, Clone, Default)]
pub struct TunnelOptions {
    /// Override the transport protocol (e.g. "http2").
    pub protocol: Option<String>,
    /// Override the edge region (e.g. "us").
    pub region: Option<String>,
    /// Static edge IP:port addresses — bypasses DNS discovery entirely.
    /// Maps to cloudflared's hidden `--edge` flag / `TUNNEL_EDGE` env var.
    pub edge_ips: Vec<String>,
}

// ── Public entry point ──────────────────────────────────────────────

/// Spawn the `cloudflared` tunnel process.
///
/// For **local-managed** tunnels the function ensures a certificate, tunnel,
/// and DNS record exist (prompting for browser login if needed), generates
/// a config file, then starts cloudflared.
///
/// For **quick** tunnels it waits (up to 15 s) for the `trycloudflare.com`
/// URL to appear in stderr and prints it.
///
/// Returns a `TunnelHandle` whose `Drop` kills the child process.
pub async fn spawn_tunnel(mode: TunnelMode, local_port: u16, opts: &TunnelOptions) -> TunnelHandle {
    let result = match &mode {
        TunnelMode::LocalManaged {
            hostname,
            tunnel_name,
        } => spawn_local_managed(hostname, tunnel_name, local_port, opts).await,
        TunnelMode::Named { token } => spawn_named(token, local_port, opts).await,
        TunnelMode::Quick => spawn_quick(local_port, opts).await,
    };

    match result {
        Ok((child, config_file)) => TunnelHandle {
            child: Some(child),
            _config_file: config_file,
        },
        Err(e) => {
            error!("Failed to start cloudflared tunnel: {e}");
            TunnelHandle {
                child: None,
                _config_file: None,
            }
        }
    }
}
