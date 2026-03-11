//! Local-managed tunnel setup helpers — create tunnel, route DNS, generate config.

use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use super::local_managed::read_tunnel_uuid;
use super::types::TunnelOptions;

/// Run `cloudflared tunnel create <name>` and save credentials.
pub(crate) async fn create_tunnel(
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
pub(crate) async fn route_dns(cert_path: &Path, tunnel_uuid: &str, hostname: &str) -> anyhow::Result<()> {
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
pub(crate) fn generate_config(
    config_path: &Path,
    tunnel_uuid: &str,
    tunnel_json_path: &Path,
    hostname: &str,
    local_port: u16,
    opts: &TunnelOptions,
) -> anyhow::Result<()> {
    let mut config = serde_json::json!({
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
    // When a protocol override is requested, bake it into the config file as well
    if let Some(ref proto) = opts.protocol {
        config.as_object_mut().unwrap().insert(
            "protocol".to_string(),
            serde_json::Value::String(proto.clone()),
        );
    }
    if let Some(ref region) = opts.region {
        config.as_object_mut().unwrap().insert(
            "region".to_string(),
            serde_json::Value::String(region.clone()),
        );
    }

    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(config_path, &content)?;
    info!("Generated tunnel config at {}", config_path.display());
    debug!("Config content: {content}");
    Ok(())
}

/// Verify that a tunnel with the given UUID still exists and matches the name.
pub(crate) async fn verify_tunnel(
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
