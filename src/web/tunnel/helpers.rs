//! Shared helpers for cloudflared command construction.

use tokio::process::Command;

use super::types::TunnelOptions;

/// Append protocol/region/edge flags to a cloudflared arg vector.
pub(crate) fn apply_tunnel_opts_to_args(args: &mut Vec<String>, opts: &TunnelOptions) {
    if let Some(ref proto) = opts.protocol {
        args.extend([
            "--protocol".to_string(),
            proto.clone(),
            "--edge-ip-version".to_string(),
            "4".to_string(),
        ]);
    }
    if let Some(ref region) = opts.region {
        args.extend(["--region".to_string(), region.clone()]);
    }
    // Static edge addresses — each gets its own --edge flag.
    // This bypasses DNS discovery entirely inside cloudflared.
    for addr in &opts.edge_ips {
        args.extend(["--edge".to_string(), addr.clone()]);
    }
}

/// Set environment variables on a `Command` for protocol/region/edge overrides.
pub(crate) fn apply_tunnel_opts_to_env(cmd: &mut Command, opts: &TunnelOptions) {
    if let Some(ref proto) = opts.protocol {
        cmd.env("TUNNEL_TRANSPORT_PROTOCOL", proto);
        cmd.env("TUNNEL_EDGE_IP_VERSION", "4");
    }
    if let Some(ref region) = opts.region {
        cmd.env("TUNNEL_REGION", region);
    }
    // TUNNEL_EDGE env var accepts comma-separated addresses
    if !opts.edge_ips.is_empty() {
        cmd.env("TUNNEL_EDGE", opts.edge_ips.join(","));
    }
}

/// Extract a tunnel URL from a cloudflared log line.
pub(crate) fn extract_tunnel_url(line: &str) -> Option<String> {
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

/// Extract a Cloudflare auth/login URL from a cloudflared log line.
///
/// cloudflared prints lines like:
///   `Please open the following URL and log in with your Cloudflare account:`
///   `https://dash.cloudflare.com/argotunnel?aud=&callback=https%3A...`
/// or sometimes embedded in an INF log line.
pub(crate) fn extract_auth_url(line: &str) -> Option<String> {
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
