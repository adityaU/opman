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

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

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
fn tunnel_data_dir() -> anyhow::Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine config directory"))?
        .join("opman")
        .join("tunnel");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
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
pub async fn spawn_tunnel(mode: TunnelMode, local_port: u16, protocol: Option<&str>) -> TunnelHandle {
    let result = match &mode {
        TunnelMode::LocalManaged {
            hostname,
            tunnel_name,
        } => spawn_local_managed(hostname, tunnel_name, local_port, protocol).await,
        TunnelMode::Named { token } => spawn_named(token, local_port, protocol).await,
        TunnelMode::Quick => spawn_quick(local_port, protocol).await,
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

// ── Local-managed tunnel ────────────────────────────────────────────

/// Full local-managed tunnel flow:
/// 1. Ensure `cert.pem` exists (run `cloudflared tunnel login` if not)
/// 2. Ensure tunnel exists (run `cloudflared tunnel create` if not)
/// 3. Route DNS for the hostname
/// 4. Generate config.json with ingress rules
/// 5. Start `cloudflared tunnel --origincert --config run <name>`
async fn spawn_local_managed(
    hostname: &str,
    tunnel_name: &str,
    local_port: u16,
    protocol: Option<&str>,
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
    )?;

    // Step 5: Run tunnel
    info!(
        "Starting locally-managed Cloudflare tunnel: {} → http://localhost:{}",
        hostname, local_port
    );

    let mut args = vec![
        "--no-autoupdate",
        "--origincert",
        cert_path.to_str().unwrap_or_default(),
        "--config",
        config_path.to_str().unwrap_or_default(),
        "tunnel",
    ];
    if let Some(proto) = protocol {
        args.extend(["--protocol", proto]);
    }
    args.extend(["run", tunnel_name]);

    let mut child = Command::new("cloudflared")
        .args(&args)
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

/// Extract a Cloudflare auth/login URL from a cloudflared log line.
///
/// cloudflared prints lines like:
///   `Please open the following URL and log in with your Cloudflare account:`
///   `https://dash.cloudflare.com/argotunnel?aud=&callback=https%3A...`
/// or sometimes embedded in an INF log line.
fn extract_auth_url(line: &str) -> Option<String> {
    // Look for https://dash.cloudflare.com or https://login.cloudflareaccess.org
    for prefix in &[
        "https://dash.cloudflare.com",
        "https://login.cloudflareaccess.org",
    ] {
        if let Some(start) = line.find(prefix) {
            let rest = &line[start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '|' || c == '"' || c == '\'')
                .unwrap_or(rest.len());
            let url = rest[..end].trim();
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// Read the TunnelID from tunnel.json
fn read_tunnel_uuid(tunnel_json_path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(tunnel_json_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    json.get("TunnelID")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("tunnel.json missing TunnelID field"))
}

/// Verify that a tunnel with the given UUID still exists and matches the name.
async fn verify_tunnel(
    cert_path: &Path,
    tunnel_uuid: &str,
    expected_name: &str,
) -> anyhow::Result<()> {
    let output = Command::new("cloudflared")
        .args([
            "--origincert",
            cert_path.to_str().unwrap_or_default(),
            "tunnel",
            "list",
            "--output=json",
            "--id",
            tunnel_uuid,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("tunnel list failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tunnels: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;

    if tunnels.is_empty() {
        return Err(anyhow::anyhow!("tunnel {tunnel_uuid} not found on Cloudflare"));
    }

    let name = tunnels[0]
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if name != expected_name {
        return Err(anyhow::anyhow!(
            "tunnel name mismatch: expected '{expected_name}', found '{name}'"
        ));
    }

    Ok(())
}

/// Run `cloudflared tunnel create <name>` and save credentials.
async fn create_tunnel(
    cert_path: &Path,
    tunnel_json_path: &Path,
    tunnel_name: &str,
) -> anyhow::Result<String> {
    info!("Creating new Cloudflare tunnel '{tunnel_name}'...");

    let status = Command::new("cloudflared")
        .args([
            "--origincert",
            cert_path.to_str().unwrap_or_default(),
            "--cred-file",
            tunnel_json_path.to_str().unwrap_or_default(),
            "tunnel",
            "create",
            tunnel_name,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to create tunnel '{tunnel_name}'.\n\
             If a tunnel with this name already exists, either:\n\
             - Delete it in the Cloudflare Zero Trust dashboard\n\
             - Or use a different --tunnel-name"
        ));
    }

    let uuid = read_tunnel_uuid(tunnel_json_path)?;
    info!("Created tunnel '{tunnel_name}' with UUID {uuid}");
    Ok(uuid)
}

/// Run `cloudflared tunnel route dns <uuid> <hostname>` to create/update CNAME.
async fn route_dns(cert_path: &Path, tunnel_uuid: &str, hostname: &str) -> anyhow::Result<()> {
    info!("Creating DNS record: {hostname} → tunnel {tunnel_uuid}");

    let status = Command::new("cloudflared")
        .args([
            "--origincert",
            cert_path.to_str().unwrap_or_default(),
            "tunnel",
            "route",
            "dns",
            "-f", // force overwrite existing record
            tunnel_uuid,
            hostname,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to create DNS entry for {hostname}.\n\
             Make sure the domain is active in your Cloudflare account."
        ));
    }

    info!("DNS record created/updated for {hostname}");
    Ok(())
}

/// Generate config.json for cloudflared with the correct ingress rules.
fn generate_config(
    config_path: &Path,
    tunnel_uuid: &str,
    tunnel_json_path: &Path,
    hostname: &str,
    local_port: u16,
) -> anyhow::Result<()> {
    let config = serde_json::json!({
        "tunnel": tunnel_uuid,
        "credentials-file": tunnel_json_path.to_str().unwrap_or_default(),
        "ingress": [
            {
                "hostname": hostname,
                "service": format!("http://localhost:{local_port}"),
                "originRequest": {
                    "noTLSVerify": true
                }
            },
            {
                "service": "http_status:404"
            }
        ]
    });

    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, &content)?;
    info!("Generated tunnel config at {}", config_path.display());
    debug!("Config content: {content}");
    Ok(())
}

// ── Remote-managed (named) tunnel ───────────────────────────────────

/// `cloudflared tunnel run --token <token>`
async fn spawn_named(token: &str, local_port: u16, protocol: Option<&str>) -> anyhow::Result<(Child, Option<PathBuf>)> {
    info!(
        "Starting named Cloudflare tunnel (port {})",
        local_port
    );

    let mut args = vec!["tunnel"];
    if let Some(proto) = protocol {
        args.extend(["--protocol", proto]);
    }
    args.extend(["run", "--token", token]);

    let mut child = Command::new("cloudflared")
        .args(&args)
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

// ── Quick (ephemeral) tunnel ────────────────────────────────────────

/// `cloudflared tunnel --url http://localhost:<port>`
async fn spawn_quick(local_port: u16, protocol: Option<&str>) -> anyhow::Result<(Child, Option<PathBuf>)> {
    info!(
        "Starting quick Cloudflare tunnel → http://localhost:{local_port}"
    );

    let local_url = format!("http://localhost:{local_port}");
    let mut args = vec!["tunnel"];
    if let Some(proto) = protocol {
        args.extend(["--protocol", proto]);
    }
    args.extend(["--url", &local_url]);

    let mut child = Command::new("cloudflared")
        .args(&args)
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

// ── Helpers ─────────────────────────────────────────────────────────

/// Extract a tunnel URL from a cloudflared log line.
fn extract_tunnel_url(line: &str) -> Option<String> {
    if let Some(start) = line.find("https://") {
        let rest = &line[start..];
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

    #[test]
    fn test_tunnel_data_dir() {
        let dir = tunnel_data_dir().unwrap();
        assert!(dir.ends_with("opman/tunnel"));
    }

    #[test]
    fn test_generate_config() {
        let tmp = std::env::temp_dir().join("opman_test_config.json");
        generate_config(
            &tmp,
            "test-uuid-1234",
            Path::new("/tmp/tunnel.json"),
            "test.example.com",
            8080,
        )
        .unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&tmp).unwrap()).unwrap();
        assert_eq!(content["tunnel"], "test-uuid-1234");
        assert_eq!(content["ingress"][0]["hostname"], "test.example.com");
        assert_eq!(
            content["ingress"][0]["service"],
            "http://localhost:8080"
        );
        assert_eq!(content["ingress"][1]["service"], "http_status:404");

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_extract_auth_url() {
        // Typical cloudflared login output
        let line = "Please open the following URL and log in with your Cloudflare account: https://dash.cloudflare.com/argotunnel?aud=abc&callback=https%3A%2F%2Flogin";
        assert_eq!(
            extract_auth_url(line),
            Some("https://dash.cloudflare.com/argotunnel?aud=abc&callback=https%3A%2F%2Flogin".to_string())
        );

        // No auth URL
        assert_eq!(extract_auth_url("some random log line"), None);
        assert_eq!(extract_auth_url("https://example.com is not cloudflare"), None);
    }
}
