//! Cloudflare Tunnel integration.
//!
//! Spawns `cloudflared` as a child process to expose the web UI via a
//! Cloudflare tunnel.  Two modes are supported:
//!
//! - **Named tunnel**: Set `OPMAN_CF_TUNNEL_TOKEN=<token>` to run a
//!   pre-configured tunnel (`cloudflared tunnel run --token <token>`).
//!   The hostname is whatever you configured in the Cloudflare dashboard.
//!
//! - **Quick tunnel**: Set `OPMAN_CF_TUNNEL=1` (or `quick`) to run an
//!   ephemeral tunnel (`cloudflared tunnel --url http://localhost:<port>`).
//!   A temporary `trycloudflare.com` URL is printed to stdout.
//!
//! The tunnel process is killed when the returned `TunnelHandle` is dropped.

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

/// Which tunnel mode to use.
#[derive(Debug, Clone)]
pub enum TunnelMode {
    /// Named tunnel — `cloudflared tunnel run --token <token>`.
    Named { token: String },
    /// Quick (ephemeral) tunnel — `cloudflared tunnel --url http://localhost:<port>`.
    Quick,
}

/// Returned by [`spawn_tunnel`]. Killing the child on drop.
pub struct TunnelHandle {
    child: Option<Child>,
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            // Best-effort kill — the process may already be dead.
            let _ = child.start_kill();
            info!("Cloudflare tunnel process killed");
        }
    }
}

/// Detect tunnel configuration from environment variables.
///
/// Returns `Some(mode)` if a tunnel should be started, `None` otherwise.
///
/// Checked variables (in order):
/// 1. `OPMAN_CF_TUNNEL_TOKEN` — if set, use named-tunnel mode.
/// 2. `OPMAN_CF_TUNNEL` — if set to any truthy value (`1`, `true`, `yes`,
///    `quick`), use quick-tunnel mode.
pub fn detect_tunnel_mode() -> Option<TunnelMode> {
    if let Ok(token) = std::env::var("OPMAN_CF_TUNNEL_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Some(TunnelMode::Named { token });
        }
    }
    if let Ok(val) = std::env::var("OPMAN_CF_TUNNEL") {
        let val = val.trim().to_lowercase();
        if matches!(val.as_str(), "1" | "true" | "yes" | "quick" | "on") {
            return Some(TunnelMode::Quick);
        }
    }
    None
}

/// Spawn the `cloudflared` tunnel process.
///
/// For quick tunnels the function waits (up to 15 s) for the
/// `trycloudflare.com` URL to appear in stderr and prints it.
///
/// Returns a `TunnelHandle` whose `Drop` kills the child process.
pub async fn spawn_tunnel(mode: TunnelMode, local_port: u16) -> TunnelHandle {
    let result = match &mode {
        TunnelMode::Named { token } => spawn_named(token, local_port).await,
        TunnelMode::Quick => spawn_quick(local_port).await,
    };

    match result {
        Ok(child) => TunnelHandle { child: Some(child) },
        Err(e) => {
            error!("Failed to start cloudflared tunnel: {e}");
            TunnelHandle { child: None }
        }
    }
}

/// `cloudflared tunnel run --token <token>`
async fn spawn_named(token: &str, local_port: u16) -> anyhow::Result<Child> {
    info!(
        "Starting named Cloudflare tunnel (port {})",
        local_port
    );

    let child = Command::new("cloudflared")
        .args(["tunnel", "run", "--token", token])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let child_id = child.id().unwrap_or(0);
    info!("cloudflared (named) started — pid {child_id}");

    // Drain stderr in background so the pipe doesn't block.
    let stderr = child.stderr.as_ref().map(|_| ()).is_some();
    if stderr {
        // We need to take ownership, but Child doesn't let us take stderr
        // after spawn easily.  We'll use a slightly different approach:
        // re-create with owned stderr.
    }

    // Actually, let's take stderr properly by re-structuring:
    // Since we already spawned, let's just spawn a log-drainer task.
    let mut child = child;
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("[cloudflared] {line}");
                // Print connection info to the user
                if line.contains("Registered tunnel connection")
                    || line.contains("Connection registered")
                {
                    info!("[tunnel] {line}");
                    println!("[tunnel] {line}");
                }
            }
        });
    }

    Ok(child)
}

/// `cloudflared tunnel --url http://localhost:<port>`
async fn spawn_quick(local_port: u16) -> anyhow::Result<Child> {
    info!(
        "Starting quick Cloudflare tunnel → http://localhost:{local_port}"
    );

    let mut child = Command::new("cloudflared")
        .args([
            "tunnel",
            "--url",
            &format!("http://localhost:{local_port}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let child_id = child.id().unwrap_or(0);
    info!("cloudflared (quick) started — pid {child_id}");

    // cloudflared prints the public URL to stderr.
    // Wait up to 15 seconds for it, then keep draining in the background.
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stderr from cloudflared"))?;

    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    let mut url_found = false;

    // Read lines with a timeout to find the URL.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            warn!("Timed out waiting for cloudflared to print tunnel URL");
            break;
        }
        match tokio::time::timeout(remaining, lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                debug!("[cloudflared] {line}");
                // Look for the trycloudflare.com URL
                if let Some(url) = extract_tunnel_url(&line) {
                    println!("\n  Tunnel URL: {url}\n");
                    info!("Quick tunnel URL: {url}");
                    url_found = true;
                    break;
                }
            }
            Ok(Ok(None)) => {
                // EOF — process exited
                warn!("cloudflared exited before printing tunnel URL");
                break;
            }
            Ok(Err(e)) => {
                warn!("Error reading cloudflared stderr: {e}");
                break;
            }
            Err(_) => {
                warn!("Timed out waiting for cloudflared to print tunnel URL");
                break;
            }
        }
    }

    if !url_found {
        warn!("Could not detect tunnel URL — tunnel may still be starting");
    }

    // Keep draining stderr in background
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[cloudflared] {line}");
        }
    });

    Ok(child)
}

/// Extract a tunnel URL from a cloudflared log line.
///
/// cloudflared prints lines like:
///   `+---...---+`
///   `|  https://foo-bar-baz.trycloudflare.com  |`
///   `+---...---+`
/// or:
///   `INF ... url=https://foo-bar-baz.trycloudflare.com`
fn extract_tunnel_url(line: &str) -> Option<String> {
    // Method 1: `https://...trycloudflare.com` anywhere in the line
    if let Some(start) = line.find("https://") {
        let rest = &line[start..];
        // Take until whitespace or | or end
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '|')
            .unwrap_or(rest.len());
        let url = rest[..end].trim();
        if url.contains("trycloudflare.com") || url.contains("cfargotunnel.com") {
            return Some(url.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_url_from_table() {
        let line = "|  https://foo-bar-baz.trycloudflare.com  |";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://foo-bar-baz.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_from_log() {
        let line =
            "2025-03-05T10:00:00Z INF +---https://abc-123.trycloudflare.com registered";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://abc-123.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_no_url() {
        assert_eq!(extract_tunnel_url("some random log line"), None);
        assert_eq!(
            extract_tunnel_url("https://example.com is not a tunnel"),
            None
        );
    }
}
