//! Generated tests for local-managed setup helpers.
//!
//! `create_tunnel`, `route_dns`, and `verify_tunnel` all spawn `cloudflared`
//! and are not exercised here (see module report). `generate_config` is pure.

use super::*;

fn read_json(p: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn generate_config_basic_ingress() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.json");
    let opts = TunnelOptions::default();
    generate_config(
        &cfg,
        "uuid-1",
        Path::new("/data/tunnel.json"),
        "host.example.com",
        9090,
        &opts,
    )
    .unwrap();

    let v = read_json(&cfg);
    assert_eq!(v["tunnel"], "uuid-1");
    assert_eq!(v["credentials-file"], "/data/tunnel.json");
    assert_eq!(v["ingress"][0]["hostname"], "host.example.com");
    assert_eq!(v["ingress"][0]["service"], "http://localhost:9090");
    assert_eq!(v["ingress"][0]["originRequest"]["noTLSVerify"], true);
    assert_eq!(v["ingress"][1]["service"], "http_status:404");
    // No protocol/region keys when opts are empty.
    assert!(v.get("protocol").is_none());
    assert!(v.get("region").is_none());
}

#[test]
fn generate_config_includes_protocol_and_region() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.json");
    let opts = TunnelOptions {
        protocol: Some("quic".into()),
        region: Some("us".into()),
        edge_ips: vec![],
    };
    generate_config(
        &cfg,
        "u2",
        Path::new("/t.json"),
        "h.example.com",
        8080,
        &opts,
    )
    .unwrap();

    let v = read_json(&cfg);
    assert_eq!(v["protocol"], "quic");
    assert_eq!(v["region"], "us");
}

#[test]
fn generate_config_only_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("config.json");
    let opts = TunnelOptions {
        protocol: Some("http2".into()),
        region: None,
        edge_ips: vec![],
    };
    generate_config(&cfg, "u3", Path::new("/t.json"), "h", 80, &opts).unwrap();
    let v = read_json(&cfg);
    assert_eq!(v["protocol"], "http2");
    assert!(v.get("region").is_none());
}

#[test]
fn generate_config_errors_on_unwritable_path() {
    let opts = TunnelOptions::default();
    // Parent directory does not exist -> std::fs::write fails -> Err.
    let res = generate_config(
        Path::new("/nonexistent-dir-xyz/config.json"),
        "u",
        Path::new("/t.json"),
        "h",
        80,
        &opts,
    );
    assert!(res.is_err());
}
