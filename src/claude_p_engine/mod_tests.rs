use super::*;
use std::collections::HashMap as StdHashMap;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

#[test]
fn now_ms_is_nonzero_and_increasing() {
    let a = now_ms();
    let b = now_ms();
    assert!(a > 0);
    assert!(b >= a);
}

#[test]
fn rand_id_has_prefix_and_hex_len() {
    let id = rand_id("ses");
    assert!(id.starts_with("ses_"));
    // 32 hex chars after the underscore.
    let hex = id.strip_prefix("ses_").unwrap();
    assert_eq!(hex.len(), 32);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    // Two calls differ.
    assert_ne!(rand_id("x"), rand_id("x"));
}

#[test]
fn default_model_reads_env_and_filters_empty() {
    let _env_guard = crate::claude_engine::claude_cli::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_MODEL").ok();
    std::env::set_var("OPMAN_CLAUDE_MODEL", "my-model");
    assert_eq!(default_model(), Some("my-model".to_string()));
    std::env::set_var("OPMAN_CLAUDE_MODEL", "");
    assert_eq!(default_model(), None);
    std::env::remove_var("OPMAN_CLAUDE_MODEL");
    assert_eq!(default_model(), None);
    if let Some(p) = prev {
        std::env::set_var("OPMAN_CLAUDE_MODEL", p);
    }
}

#[test]
fn new_sets_default_mode_from_env_or_bypass() {
    let prev = std::env::var("OPMAN_CLAUDE_PERMISSION_MODE").ok();
    std::env::remove_var("OPMAN_CLAUDE_PERMISSION_MODE");
    let e = ClaudePEngine::new(None, (true, false, false, false));
    assert_eq!(e.default_mode, "bypassPermissions");
    assert_eq!(e.mcp_flags, (true, false, false, false));
    assert!(e.persist.is_none());
    if let Some(p) = prev {
        std::env::set_var("OPMAN_CLAUDE_PERMISSION_MODE", p);
    }
}

#[tokio::test]
async fn emit_and_subscribe_roundtrip() {
    let e = engine();
    let mut rx = e.subscribe();
    e.emit("dir-a", "custom.event", json!({ "k": 1 }));
    let ev = rx.recv().await.unwrap();
    assert_eq!(ev.directory, "dir-a");
    let v: Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "custom.event");
    assert_eq!(v["properties"]["k"], 1);
}

#[test]
fn sessions_get_mut_trait_impl() {
    let mut map: StdHashMap<String, Session> = StdHashMap::new();
    map.insert(
        "id1".to_string(),
        Session {
            id: "id1".into(),
            ..Default::default()
        },
    );
    assert!(map.sessions_get_mut("id1").is_some());
    assert!(map.sessions_get_mut("missing").is_none());
    map.sessions_get_mut("id1").unwrap().busy = true;
    assert!(map.get("id1").unwrap().busy);
}

#[tokio::test]
async fn start_embedded_server_binds_and_returns_url() {
    let (url, _handle) = start_embedded_server((false, false, false, false))
        .await
        .unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
}
