//! Remote-managed (named) Cloudflare tunnel.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, info};

use super::helpers::{apply_tunnel_opts_to_args, apply_tunnel_opts_to_env};
use super::types::TunnelOptions;

/// `cloudflared tunnel run --token <token>`
pub(crate) async fn spawn_named(token: &str, local_port: u16, opts: &TunnelOptions) -> anyhow::Result<(Child, Option<PathBuf>)> {
    info!(
        "Starting named Cloudflare tunnel (port {})",
        local_port
    );

    let mut args = vec!["tunnel".to_string()];
    apply_tunnel_opts_to_args(&mut args, opts);
    args.extend(["run".to_string(), "--token".to_string(), token.to_string()]);

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
    info!("cloudflared (named) started — pid {child_id}");

    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("[cloudflared] {line}");
                if line.contains("Registered tunnel connection")
                    || line.contains("Connection registered")
                {
                    info!("[tunnel] {line}");
                    println!("[tunnel] {line}");
                }
            }
        });
    }

    Ok((child, None))
}
