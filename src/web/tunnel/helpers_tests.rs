//! Generated tests for cloudflared command-construction helpers.

use super::*;
use crate::web::tunnel::types::TunnelOptions;

fn opts(protocol: Option<&str>, region: Option<&str>, edges: &[&str]) -> TunnelOptions {
    TunnelOptions {
        protocol: protocol.map(|s| s.to_string()),
        region: region.map(|s| s.to_string()),
        edge_ips: edges.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn apply_args_empty_opts_adds_nothing() {
    let mut args = vec!["tunnel".to_string()];
    apply_tunnel_opts_to_args(&mut args, &opts(None, None, &[]));
    assert_eq!(args, vec!["tunnel".to_string()]);
}

#[test]
fn apply_args_protocol_adds_protocol_and_edge_ip_version() {
    let mut args = Vec::new();
    apply_tunnel_opts_to_args(&mut args, &opts(Some("http2"), None, &[]));
    assert_eq!(
        args,
        vec![
            "--protocol".to_string(),
            "http2".to_string(),
            "--edge-ip-version".to_string(),
            "4".to_string()
        ]
    );
}

#[test]
fn apply_args_region_and_edges() {
    let mut args = Vec::new();
    apply_tunnel_opts_to_args(&mut args, &opts(None, Some("us"), &["1.2.3.4:7844", "5.6.7.8:7844"]));
    assert_eq!(
        args,
        vec![
            "--region", "us",
            "--edge", "1.2.3.4:7844",
            "--edge", "5.6.7.8:7844",
        ]
    );
}

#[test]
fn apply_args_all_options_combined() {
    let mut args = Vec::new();
    apply_tunnel_opts_to_args(&mut args, &opts(Some("quic"), Some("eu"), &["9.9.9.9:7844"]));
    // protocol block first, then region, then edges.
    assert_eq!(args[0], "--protocol");
    assert!(args.contains(&"--region".to_string()));
    assert!(args.contains(&"eu".to_string()));
    assert!(args.contains(&"9.9.9.9:7844".to_string()));
}

#[test]
fn apply_env_variants_do_not_panic() {
    // apply_tunnel_opts_to_env just sets env vars on the Command; exercise every branch.
    let mut cmd = tokio::process::Command::new("true");
    apply_tunnel_opts_to_env(&mut cmd, &opts(Some("http2"), Some("us"), &["1.1.1.1:7844", "2.2.2.2:7844"]));

    let mut cmd2 = tokio::process::Command::new("true");
    apply_tunnel_opts_to_env(&mut cmd2, &opts(None, None, &[]));
}

#[test]
fn extract_tunnel_url_trycloudflare_and_cfargo() {
    assert_eq!(
        extract_tunnel_url("INF |  https://foo-bar.trycloudflare.com  |"),
        Some("https://foo-bar.trycloudflare.com".to_string())
    );
    assert_eq!(
        extract_tunnel_url("see https://abc.cfargotunnel.com now"),
        Some("https://abc.cfargotunnel.com".to_string())
    );
}

#[test]
fn extract_tunnel_url_pipe_terminated() {
    // The URL is terminated by a '|' with no surrounding whitespace.
    assert_eq!(
        extract_tunnel_url("x=https://z.trycloudflare.com|rest"),
        Some("https://z.trycloudflare.com".to_string())
    );
}

#[test]
fn extract_tunnel_url_rejects_non_tunnel_hosts_and_missing() {
    assert_eq!(extract_tunnel_url("plain log with no url"), None);
    assert_eq!(extract_tunnel_url("https://example.com not a tunnel"), None);
}

#[test]
fn extract_auth_url_dash_and_access() {
    assert_eq!(
        extract_auth_url("Please open: https://dash.cloudflare.com/argotunnel?aud=x here"),
        Some("https://dash.cloudflare.com/argotunnel?aud=x".to_string())
    );
    assert_eq!(
        extract_auth_url("go to https://login.cloudflareaccess.org/abc\""),
        Some("https://login.cloudflareaccess.org/abc".to_string())
    );
}

#[test]
fn extract_auth_url_quote_terminated() {
    assert_eq!(
        extract_auth_url("url='https://dash.cloudflare.com/x'"),
        Some("https://dash.cloudflare.com/x".to_string())
    );
}

#[test]
fn extract_auth_url_none_when_absent() {
    assert_eq!(extract_auth_url("nothing here"), None);
    assert_eq!(extract_auth_url("https://example.com/login"), None);
}
