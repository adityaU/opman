use super::*;

use std::sync::Mutex;

/// Serialize env-mutating tests in this module (env vars are process-global).
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK;

#[test]
fn web_config_construction() {
    let cfg = WebConfig {
        port: Some(8080),
        username: "admin".to_string(),
        password: "secret".to_string(),
        instance_name: Some("laptop".to_string()),
        backend: "claude-code".to_string(),
    };
    assert_eq!(cfg.port, Some(8080));
    assert_eq!(cfg.username, "admin");
    assert_eq!(cfg.password, "secret");
    assert_eq!(cfg.instance_name.as_deref(), Some("laptop"));
    assert_eq!(cfg.backend, "claude-code");
}

#[test]
fn web_config_minimal() {
    let cfg = WebConfig {
        port: None,
        username: String::new(),
        password: String::new(),
        instance_name: None,
        backend: "opencode".to_string(),
    };
    assert!(cfg.port.is_none());
    assert!(cfg.username.is_empty());
    assert!(cfg.instance_name.is_none());
    assert_eq!(cfg.backend, "opencode");
}

#[test]
fn write_internal_descriptor_writes_valid_json() {
    let _guard = ENV_LOCK.lock().unwrap();

    // Point the config dir at a unique temp directory via XDG_CONFIG_HOME,
    // which `dirs::config_dir()` honours on Linux.
    let unique = format!("opman_mod_test_{}", rand::random::<u64>());
    let tmp = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&tmp).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_home = std::env::var_os("HOME");
    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    std::env::set_var("HOME", &tmp);

    write_internal_descriptor(54321, "deadbeef-token");

    let written = tmp.join("opman").join("internal.json");
    let result = std::fs::read_to_string(&written);

    // Restore env before asserting so a failure never leaks state.
    match prev_xdg {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    match prev_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }

    let contents = result.expect("internal.json should be written");
    let json: serde_json::Value = serde_json::from_str(&contents).expect("valid json");
    assert_eq!(json["url"], "http://127.0.0.1:54321");
    assert_eq!(json["token"], "deadbeef-token");

    let _ = std::fs::remove_dir_all(&tmp);
}
