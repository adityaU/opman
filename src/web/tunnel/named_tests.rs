//! Generated tests for the named-tunnel arg builder.
//!
//! `spawn_named` spawns `cloudflared` and is not exercised here.

use super::*;

#[test]
fn build_named_args_default() {
    let args = build_named_args("my-token", &TunnelOptions::default());
    assert_eq!(
        args,
        vec!["tunnel", "run", "--token", "my-token"]
    );
}

#[test]
fn build_named_args_with_opts() {
    let opts = TunnelOptions {
        protocol: Some("http2".into()),
        region: Some("us".into()),
        edge_ips: vec!["1.2.3.4:7844".into()],
    };
    let args = build_named_args("tok", &opts);
    // tunnel <opts...> run --token tok
    assert_eq!(args[0], "tunnel");
    assert_eq!(&args[args.len() - 3..], &["run", "--token", "tok"]);
    assert!(args.contains(&"--protocol".to_string()));
    assert!(args.contains(&"--edge".to_string()));
    assert!(args.contains(&"1.2.3.4:7844".to_string()));
}
