//! Generated tests for the quick-tunnel arg builder.
//!
//! `spawn_quick` spawns `cloudflared` and waits on its stderr; it is not
//! exercised here (see module report).

use super::*;

#[test]
fn build_quick_args_default_url() {
    let args = build_quick_args(8080, &TunnelOptions::default());
    assert_eq!(args, vec!["tunnel", "--url", "http://localhost:8080"]);
}

#[test]
fn build_quick_args_with_opts_before_url() {
    let opts = TunnelOptions {
        protocol: Some("quic".into()),
        region: None,
        edge_ips: vec![],
    };
    let args = build_quick_args(3000, &opts);
    assert_eq!(args[0], "tunnel");
    assert!(args.contains(&"--protocol".to_string()));
    // --url + value come last.
    assert_eq!(&args[args.len() - 2..], &["--url", "http://localhost:3000"]);
}
