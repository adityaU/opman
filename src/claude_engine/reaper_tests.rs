//! Extra coverage for the reaper: env config, plan edge cases, engine helpers, and a
//! full `reap_once` sweep driven by a stubbed `claude` binary.
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use crate::claude_engine::registry::SessionEntry;

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` with an env var temporarily set (or removed if `val` is None), restoring it.
fn with_env<T>(key: &str, val: Option<&str>, f: impl FnOnce() -> T) -> T {
    let _g = env_guard();
    let prev = std::env::var(key).ok();
    match val {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
    let out = f();
    match prev {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
    out
}

fn agent(id: &str, uuid: &str, state: &str, started_at: u64) -> claude_cli::AgentInfo {
    claude_cli::AgentInfo {
        id: id.to_string(),
        session_id: uuid.to_string(),
        cwd: "/proj".to_string(),
        kind: "background".to_string(),
        state: Some(state.to_string()),
        status: None,
        name: String::new(),
        started_at,
    }
}

const NOW: u64 = 10_000_000;
const TTL: u64 = 300_000;
const OLD: u64 = NOW - MIN_AGE_MS - 1;

// ---- build_plan edge cases --------------------------------------------

#[test]
fn agent_with_empty_id_is_skipped() {
    let agents = vec![agent("", "u", "done", OLD)];
    assert!(build_plan(&agents, &HashMap::new(), NOW, TTL).is_empty());
}

#[test]
fn mixed_agents_only_reap_the_idle_untracked_ones() {
    let agents = vec![
        agent("busy", "u1", "working", OLD),     // busy → kept
        agent("young", "u2", "done", NOW - 100), // too young → kept
        agent("reapme", "u3", "done", OLD),      // idle + untracked → reaped
    ];
    let plan = build_plan(&agents, &HashMap::new(), NOW, TTL);
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].short_id, "reapme");
    assert_eq!(plan[0].reason, "superseded-or-untracked");
}

// ---- enabled() / ttl_ms() env handling --------------------------------

#[test]
fn enabled_reads_env() {
    assert!(
        with_env("OPMAN_CLAUDE_REAP", None, enabled),
        "default enabled"
    );
    assert!(!with_env("OPMAN_CLAUDE_REAP", Some("0"), enabled));
    assert!(with_env("OPMAN_CLAUDE_REAP", Some("1"), enabled));
}

#[test]
fn ttl_ms_reads_env() {
    let default = with_env("OPMAN_CLAUDE_AGENT_TTL_SECS", None, ttl_ms);
    assert_eq!(default, DEFAULT_TTL_SECS * 1000);
    assert_eq!(
        with_env("OPMAN_CLAUDE_AGENT_TTL_SECS", Some("120"), ttl_ms),
        120_000
    );
    // Zero is filtered → falls back to default.
    assert_eq!(
        with_env("OPMAN_CLAUDE_AGENT_TTL_SECS", Some("0"), ttl_ms),
        DEFAULT_TTL_SECS * 1000
    );
    // Unparseable → default.
    assert_eq!(
        with_env("OPMAN_CLAUDE_AGENT_TTL_SECS", Some("nope"), ttl_ms),
        DEFAULT_TTL_SECS * 1000
    );
}

// ---- reap_snapshot / clear_short_ids ----------------------------------

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ))
}

fn seed(engine: &Arc<ClaudeEngine>, e: SessionEntry) {
    engine.reg.lock().unwrap().sessions.insert(e.id.clone(), e);
}

#[test]
fn reap_snapshot_keys_by_uuid_and_flags_protected() {
    let e = engine();
    seed(
        &e,
        SessionEntry {
            id: "ses_live".into(),
            directory: "/proj".into(),
            claude_session_id: Some("uuid-live".into()),
            updated: 42,
            busy: true, // → protected
            ..Default::default()
        },
    );
    seed(
        &e,
        SessionEntry {
            id: "ses_no_uuid".into(),
            directory: "/proj".into(),
            claude_session_id: None, // skipped
            ..Default::default()
        },
    );
    seed(
        &e,
        SessionEntry {
            id: "sub".into(),
            directory: "/proj".into(),
            claude_session_id: Some("uuid-sub".into()),
            is_subagent: true, // skipped
            ..Default::default()
        },
    );

    let snap = e.reap_snapshot();
    assert_eq!(snap.len(), 1);
    let t = snap.get("uuid-live").expect("live session present");
    assert_eq!(t.session_id, "ses_live");
    assert_eq!(t.updated_ms, 42);
    assert!(t.protected);
}

#[test]
fn clear_short_ids_drops_ids_and_ignores_empty() {
    let e = engine();
    seed(
        &e,
        SessionEntry {
            id: "ses".into(),
            directory: "/proj".into(),
            short_id: Some("aa".into()),
            ..Default::default()
        },
    );
    // Empty input is a no-op.
    e.clear_short_ids(&[]);
    assert_eq!(
        e.get_session("ses").unwrap().short_id.as_deref(),
        Some("aa")
    );
    // Clearing drops the short_id (resume uuid retained).
    e.clear_short_ids(&["ses".into()]);
    assert!(e.get_session("ses").unwrap().short_id.is_none());
    // Unknown session id is harmless.
    e.clear_short_ids(&["ghost".into()]);
}

// ---- spawn_reaper disabled --------------------------------------------

#[test]
fn spawn_reaper_disabled_returns_without_spawning() {
    let e = engine();
    // OPMAN_CLAUDE_REAP=0 short-circuits before any tokio::spawn (so no runtime needed).
    with_env("OPMAN_CLAUDE_REAP", Some("0"), || spawn_reaper(e.clone()));
}

// ---- reap_once full sweep via a stubbed binary ------------------------

/// A fake `claude`: prints one done/background agent for `agents`, exits 0 for `stop`.
fn make_stub(session_uuid: &str) -> (String, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fake-claude");
    let body = format!(
        "#!/bin/sh\nif [ \"$1\" = \"agents\" ]; then printf '%s' '[{{\"id\":\"aa\",\"sessionId\":\"{session_uuid}\",\"cwd\":\"/proj\",\"kind\":\"background\",\"state\":\"done\",\"startedAt\":0}}]'; fi\n"
    );
    std::fs::write(&path, body).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    (path.to_string_lossy().into_owned(), dir)
}

#[tokio::test]
async fn reap_once_reaps_stale_current_target_and_clears_short_id() {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    let (bin, _dir) = make_stub("uuid-x");
    std::env::set_var("OPMAN_CLAUDE_BIN", &bin);

    let e = engine();
    // A tracked session whose current agent (uuid-x) is idle far past the TTL.
    seed(
        &e,
        SessionEntry {
            id: "ses".into(),
            directory: "/proj".into(),
            claude_session_id: Some("uuid-x".into()),
            short_id: Some("aa".into()),
            updated: 0, // long ago → past TTL
            busy: false,
            ..Default::default()
        },
    );

    let n = reap_once(&e).await;

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }

    assert_eq!(n, 1, "one stale-idle agent reaped");
    assert!(
        e.get_session("ses").unwrap().short_id.is_none(),
        "short_id cleared"
    );
}

#[tokio::test]
async fn reap_once_returns_zero_when_no_agents() {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    std::env::set_var("OPMAN_CLAUDE_BIN", "echo"); // non-JSON → empty agent list
    let e = engine();
    let n = reap_once(&e).await;
    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
    assert_eq!(n, 0);
}
