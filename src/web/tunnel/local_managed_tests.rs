//! Generated tests for local-managed tunnel pure helpers.
//!
//! `spawn_local_managed` and `ensure_certificate` spawn `cloudflared` and open
//! a browser — not exercised here (see module report). We cover `read_tunnel_uuid`
//! and the extracted `build_run_args`.

use super::*;

#[test]
fn build_run_args_without_opts() {
    let opts = TunnelOptions::default();
    let args = build_run_args(
        Path::new("/data/cert.pem"),
        Path::new("/data/config.json"),
        "opman",
        &opts,
    );
    assert_eq!(
        args,
        vec![
            "--no-autoupdate",
            "--origincert",
            "/data/cert.pem",
            "--config",
            "/data/config.json",
            "tunnel",
            "run",
            "opman",
        ]
    );
}

#[test]
fn build_run_args_with_protocol_and_region() {
    let opts = TunnelOptions {
        protocol: Some("http2".into()),
        region: Some("us".into()),
        edge_ips: vec![],
    };
    let args = build_run_args(Path::new("/c.pem"), Path::new("/cfg.json"), "myt", &opts);
    // protocol/region flags are injected between "tunnel" and "run".
    let tunnel_pos = args.iter().position(|a| a == "tunnel").unwrap();
    let run_pos = args.iter().position(|a| a == "run").unwrap();
    assert!(tunnel_pos < run_pos);
    assert!(args.contains(&"--protocol".to_string()));
    assert!(args.contains(&"http2".to_string()));
    assert!(args.contains(&"--region".to_string()));
    assert_eq!(args.last().unwrap(), "myt");
}

#[test]
fn read_tunnel_uuid_valid() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tunnel.json");
    std::fs::write(&p, r#"{"TunnelID":"abc-123-uuid","AccountTag":"x"}"#).unwrap();
    assert_eq!(read_tunnel_uuid(&p).unwrap(), "abc-123-uuid");
}

#[test]
fn read_tunnel_uuid_missing_field() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tunnel.json");
    std::fs::write(&p, r#"{"AccountTag":"x"}"#).unwrap();
    let err = read_tunnel_uuid(&p).unwrap_err().to_string();
    assert!(err.contains("missing TunnelID"), "got: {err}");
}

#[test]
fn read_tunnel_uuid_invalid_json() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tunnel.json");
    std::fs::write(&p, b"not json {{{").unwrap();
    assert!(read_tunnel_uuid(&p).is_err());
}

#[test]
fn read_tunnel_uuid_nonexistent_file() {
    let p = std::path::Path::new("/nonexistent/opman-test-tunnel.json");
    assert!(read_tunnel_uuid(p).is_err());
}

#[test]
fn read_tunnel_uuid_non_string_field() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tunnel.json");
    std::fs::write(&p, r#"{"TunnelID":12345}"#).unwrap();
    // as_str() is None for a number -> error.
    assert!(read_tunnel_uuid(&p).is_err());
}
