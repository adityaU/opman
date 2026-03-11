//! Quick (ephemeral) Cloudflare tunnel.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

use super::helpers::{apply_tunnel_opts_to_args, apply_tunnel_opts_to_env, extract_tunnel_url};
use super::types::TunnelOptions;

/// `cloudflared tunnel --url http://localhost:<port>`
pub(crate) async fn spawn_quick(local_port: u16, opts: &TunnelOptions) -> anyhow::Result<(Child, Option<PathBuf>)> {
    info!(
        "Starting quick Cloudflare tunnel → http://localhost:{local_port}"
    );

    let local_url = format!("http://localhost:{local_port}");
    let mut args = vec!["tunnel".to_string()];
    apply_tunnel_opts_to_args(&mut args, opts);
    args.extend(["--url".to_string(), local_url]);

    let mut cmd = Command::new("cloudflared");
    cmd.args(&args);
    apply_tunnel_opts_to_env(&mut cmd, opts);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let child_id = child.id().unwrap_or(0);
    info!("cloudflared (quick) started — pid {child_id}");

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stderr from cloudflared"))?;

    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    let mut url_found = false;

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
                if let Some(url) = extract_tunnel_url(&line) {
                    println!("\n  Tunnel URL: {url}\n");
                    info!("Quick tunnel URL: {url}");
                    url_found = true;
                    break;
                }
            }
            Ok(Ok(None)) => {
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

    Ok((child, None))
}
