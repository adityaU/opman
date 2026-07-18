//! Generated tests for tunnel core types.
//!
//! `spawn_tunnel` / `tunnel_data_dir` spawn `cloudflared` or touch the real
//! config dir and are not driven here (see the module report). We cover the
//! option/mode structs and the `TunnelHandle` Drop cleanup, which is pure.

use super::*;

#[test]
fn tunnel_options_default_is_all_none_empty() {
    let o = TunnelOptions::default();
    assert!(o.protocol.is_none());
    assert!(o.region.is_none());
    assert!(o.edge_ips.is_empty());
    // Clone + Debug are derived; exercise them.
    let c = o.clone();
    assert!(format!("{c:?}").contains("TunnelOptions"));
}

#[test]
fn tunnel_mode_variants_debug_and_clone() {
    let lm = TunnelMode::LocalManaged {
        hostname: "opman.example.com".into(),
        tunnel_name: "opman".into(),
    };
    let named = TunnelMode::Named { token: "tok".into() };
    let quick = TunnelMode::Quick;
    for m in [lm.clone(), named.clone(), quick.clone()] {
        // Debug formatting works for every variant.
        let _ = format!("{m:?}");
    }
    if let TunnelMode::LocalManaged { hostname, tunnel_name } = lm {
        assert_eq!(hostname, "opman.example.com");
        assert_eq!(tunnel_name, "opman");
    } else {
        panic!("wrong variant");
    }
    if let TunnelMode::Named { token } = named {
        assert_eq!(token, "tok");
    }
    assert!(matches!(quick, TunnelMode::Quick));
}

#[test]
fn drop_without_config_file_is_noop() {
    // child None + no config file -> Drop does nothing and must not panic.
    let h = TunnelHandle { child: None, _config_file: None };
    drop(h);
}

#[test]
fn drop_removes_temp_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, b"{}").unwrap();
    assert!(path.exists());

    let h = TunnelHandle {
        child: None,
        _config_file: Some(path.clone()),
    };
    drop(h);

    assert!(!path.exists(), "Drop should remove the temp config file");
}

#[test]
fn drop_with_missing_config_file_does_not_panic() {
    // Points at a path that doesn't exist — remove_file errors are ignored.
    let h = TunnelHandle {
        child: None,
        _config_file: Some(std::path::PathBuf::from("/nonexistent/opman-tunnel-xyz.json")),
    };
    drop(h);
}
