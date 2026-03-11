//! Local-managed Cloudflare tunnel — automatic setup & lifecycle.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

use super::helpers::{apply_tunnel_opts_to_args, apply_tunnel_opts_to_env, extract_auth_url};
use super::local_managed_setup::{create_tunnel, generate_config, route_dns, verify_tunnel};
use super::types::{tunnel_data_dir, TunnelOptions};

/// Full local-managed tunnel flow:
/// 1. Ensure `cert.pem` exists (run `cloudflared tunnel login` if not)
/// 2. Ensure tunnel exists (run `cloudflared tunnel create` if not)
/// 3. Route DNS for the hostname
/// 4. Generate config.json with ingress rules
/// 5. Start `cloudflared tunnel --origincert --config run <name>`
pub(crate) async fn spawn_local_managed(
    hostname: &str,
    tunnel_name: &str,
    local_port: u16,
    opts: &TunnelOptions,
) -> anyhow::Result<(Child, Option<PathBuf>)> {
    let data_dir = tunnel_data_dir()?;
    let cert_path = data_dir.join("cert.pem");
    let tunnel_json_path = data_dir.join("tunnel.json");

    // Step 1: Ensure certificate
    if !cert_path.exists() {
        ensure_certificate(&cert_path).await?;
    } else {
        info!("Existing Cloudflare certificate found at {}", cert_path.display());
    }

    // Step 2: Ensure tunnel exists
    let tunnel_uuid = if tunnel_json_path.exists() {
        let uuid = read_tunnel_uuid(&tunnel_json_path)?;
        info!("Existing tunnel found: {uuid}");

        // Verify the tunnel still exists on Cloudflare and matches our name
        if let Err(e) = verify_tunnel(&cert_path, &uuid, tunnel_name).await {
            warn!("Existing tunnel verification failed ({e}), recreating...");
            std::fs::remove_file(&tunnel_json_path).ok();
            create_tunnel(&cert_path, &tunnel_json_path, tunnel_name).await?
        } else {
            uuid
        }
    } else {
        create_tunnel(&cert_path, &tunnel_json_path, tunnel_name).await?
    };

    // Step 3: Route DNS (idempotent — uses -f to overwrite)
    route_dns(&cert_path, &tunnel_uuid, hostname).await?;

    // Step 4: Generate config file
    let config_path = data_dir.join("config.json");
    generate_config(
        &config_path,
        &tunnel_uuid,
        &tunnel_json_path,
        hostname,
        local_port,
        opts,
    )?;

    // Step 5: Run tunnel
    info!(
        "Starting locally-managed Cloudflare tunnel: {} → http://localhost:{}",
        hostname, local_port
    );

    let mut args = vec![
        "--no-autoupdate".to_string(),
        "--origincert".to_string(),
        cert_path.to_str().unwrap_or_default().to_string(),
        "--config".to_string(),
        config_path.to_str().unwrap_or_default().to_string(),
        "tunnel".to_string(),
    ];
    apply_tunnel_opts_to_args(&mut args, opts);
    args.extend(["run".to_string(), tunnel_name.to_string()]);

    info!("cloudflared args: {:?}", args);

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
    info!("cloudflared (local-managed) started — pid {child_id}");

    println!("\n  Tunnel: https://{hostname}\n");

    // Drain stderr in background — log every line so issues are visible
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Always log at info level so tunnel output appears in the log file
                info!("[cloudflared] {line}");
                if line.contains("Registered tunnel connection")
                    || line.contains("Connection registered")
                {
                    println!("[tunnel] {line}");
                }
                if line.contains("ERR") || line.contains("error") {
                    error!("[tunnel] {line}");
                }
            }
            // If we get here, cloudflared's stderr closed — process likely exited
            warn!("[tunnel] cloudflared process stderr closed — tunnel may have exited");
        });
    }

    Ok((child, Some(config_path)))
}

/// Run `cloudflared tunnel login` to get cert.pem.
/// Captures the auth URL from cloudflared's output, prints it prominently,
/// and opens it in the default browser.
async fn ensure_certificate(cert_path: &Path) -> anyhow::Result<()> {
    let data_dir = cert_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid cert path"))?;

    println!();
    println!("  Cloudflare authentication required.");
    println!("  Starting login flow...");
    println!();

    // Run cloudflared tunnel login, capturing stderr to extract the auth URL.
    // cloudflared prints the URL to stderr and then waits for the callback.
    let mut child = Command::new("cloudflared")
        .args(["tunnel", "login"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    // Read stderr in background, looking for the auth URL
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stderr from cloudflared login"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdout from cloudflared login"))?;

    // Drain stdout in background (cloudflared may write there too)
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[cloudflared login stdout] {line}");
            // Some versions print the URL on stdout
            if let Some(url) = extract_auth_url(&line) {
                println!();
                println!("  Open this URL in your browser to authenticate:");
                println!();
                println!("    {url}");
                println!();
                if let Err(e) = open::that(&url) {
                    debug!("Failed to auto-open browser: {e}");
                    println!("  (Could not open browser automatically — please open the URL manually)");
                }
            }
        }
    });

    // Read stderr for the auth URL
    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    // We need to drain stderr while the process runs (it blocks until auth completes)
    let stderr_task = tokio::spawn(async move {
        let mut found_url = false;
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[cloudflared login] {line}");
            // Print all cloudflared output so user can see progress
            println!("  [cloudflared] {line}");

            if !found_url {
                if let Some(url) = extract_auth_url(&line) {
                    println!();
                    println!("  ┌─────────────────────────────────────────────────────────────┐");
                    println!("  │  Open this URL in your browser to authenticate:             │");
                    println!("  └─────────────────────────────────────────────────────────────┘");
                    println!();
                    println!("    {url}");
                    println!();
                    // Try to open in browser
                    if let Err(e) = open::that(&url) {
                        debug!("Failed to auto-open browser: {e}");
                        println!("  (Could not open browser automatically — please copy the URL above)");
                    } else {
                        println!("  (Opening in your default browser...)");
                    }
                    println!();
                    found_url = true;
                }
            }
        }
        found_url
    });

    // Wait for the process to complete (user authenticates in browser)
    let status = child.wait().await?;
    let _url_shown = stderr_task.await.unwrap_or(false);

    if !status.success() {
        return Err(anyhow::anyhow!(
            "cloudflared tunnel login failed (exit code: {:?})",
            status.code()
        ));
    }

    // cloudflared writes cert.pem to ~/.cloudflared/cert.pem
    let default_cert = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
        .join(".cloudflared")
        .join("cert.pem");

    if default_cert.exists() {
        std::fs::create_dir_all(data_dir)?;
        std::fs::copy(&default_cert, cert_path)?;
        info!(
            "Moved certificate from {} to {}",
            default_cert.display(),
            cert_path.display()
        );
    } else if !cert_path.exists() {
        return Err(anyhow::anyhow!(
            "cloudflared login succeeded but cert.pem was not found at {} or {}",
            default_cert.display(),
            cert_path.display()
        ));
    }

    println!("  Authentication successful!");
    println!();
    Ok(())
}

/// Read the TunnelID from tunnel.json
pub(crate) fn read_tunnel_uuid(tunnel_json_path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(tunnel_json_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    json.get("TunnelID")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("tunnel.json missing TunnelID field"))
}


