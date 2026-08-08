//! Coverage for `start_web_server` — executes most of the startup body:
//! PTY manager start, skills registry load, config load, web state creation,
//! router build, port-0 bind + descriptor write, and the background serve task.
//!
//! All config/data/HOME-derived paths (`dirs::config_dir()`) are redirected to a
//! unique temp dir under a serializing mutex so we never touch the real
//! `~/.config/opman`.

use super::*;

use std::sync::Mutex;

fn runner_registry() -> std::sync::Arc<crate::runner::RunnerRegistry> {
    let mut runners = std::collections::HashMap::new();
    runners.insert(
        crate::runner::RunnerKind::Opencode,
        std::sync::Arc::new(crate::runner::HttpRunner::new(
            crate::runner::RunnerKind::Opencode,
            "http://127.0.0.1:9",
            reqwest::Client::new(),
        )) as std::sync::Arc<dyn crate::runner::Runner>,
    );
    std::sync::Arc::new(crate::runner::RunnerRegistry::new(
        crate::runner::RunnerKind::Opencode,
        runners,
    ))
}

/// Serializes env-mutating startup tests (env vars are process-global).
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as STARTUP_ENV_LOCK;

struct EnvRedirect {
    _tmp: tempfile::TempDir,
    prev_xdg: Option<std::ffi::OsString>,
    prev_home: Option<std::ffi::OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl EnvRedirect {
    fn new() -> Self {
        let guard = STARTUP_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        std::env::set_var("HOME", tmp.path());
        EnvRedirect {
            _tmp: tmp,
            prev_xdg,
            prev_home,
            _guard: guard,
        }
    }
}

impl Drop for EnvRedirect {
    fn drop(&mut self) {
        match self.prev_xdg.take() {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match self.prev_home.take() {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[tokio::test]
async fn start_web_server_binds_random_port() {
    let _env = EnvRedirect::new();

    let config = WebConfig {
        port: Some(0), // 0 → OS-assigned free port
        username: String::new(),
        password: String::new(),
        instance_name: Some("test-instance".to_string()),
        backend: "claude-code".to_string(),
    };
    let registry = crate::mcp::new_nvim_socket_registry();

    let registry_arc = runner_registry();
    let (port, _handle) = start_web_server(
        config,
        registry,
        registry_arc.clone(),
        crate::mcp_registry::RegistryHandle::default(),
        crate::web::test_support::test_acp_supervisor(registry_arc),
    )
    .await;
    // Port 0 requested → the OS assigns a real, non-zero port.
    assert!(port > 0, "expected a real bound port, got {port}");

    // The internal descriptor should have been written into the temp config dir.
    let desc = dirs::config_dir()
        .unwrap()
        .join("opman")
        .join("internal.json");
    let contents = std::fs::read_to_string(&desc).expect("internal.json written");
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(json["url"], format!("http://127.0.0.1:{port}"));
    assert!(json["token"].as_str().unwrap().len() > 0);
}

#[tokio::test]
async fn start_web_server_explicit_port_none_defaults_to_zero() {
    let _env = EnvRedirect::new();

    // port: None → unwrap_or(0) → still an OS-assigned port.
    let config = WebConfig {
        port: None,
        username: "u".to_string(),
        password: "p".to_string(),
        instance_name: None,
        backend: "opencode".to_string(),
    };
    let registry = crate::mcp::new_nvim_socket_registry();
    let registry_arc = runner_registry();
    let (port, _handle) = start_web_server(
        config,
        registry,
        registry_arc.clone(),
        crate::mcp_registry::RegistryHandle::default(),
        crate::web::test_support::test_acp_supervisor(registry_arc),
    )
    .await;
    assert!(port > 0);
}
