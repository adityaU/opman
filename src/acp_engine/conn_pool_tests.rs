//! Process-level coverage for ACP pooling.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::acp_engine::{config, AcpEngine, Session};
use crate::mcp_registry::{BuiltinFlags, McpRegistry, RegistryHandle};

struct Harness {
    engine: Arc<AcpEngine>,
    root: tempfile::TempDir,
    count: PathBuf,
    sessions: PathBuf,
    events: PathBuf,
}

impl Harness {
    fn new(shared_process: bool) -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let script = root.path().join("fake-acp.sh");
        let count = root.path().join("initialize-count");
        let sessions = root.path().join("session-count");
        let events = root.path().join("events");
        fs::write(&script, FAKE_AGENT).expect("fake agent script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .expect("make fake agent executable");
        let agent = config::AgentConfig {
            command: script.to_string_lossy().into_owned(),
            args: vec![
                count.to_string_lossy().into_owned(),
                events.to_string_lossy().into_owned(),
                sessions.to_string_lossy().into_owned(),
            ],
            inject_mcp: false,
            shared_process,
            ..Default::default()
        };
        let flags = BuiltinFlags::default();
        let registry = RegistryHandle::new(Arc::new(McpRegistry::builtins(flags)), flags);
        let engine = Arc::new(AcpEngine::new(
            "pool-test".to_string(),
            agent,
            None,
            registry,
        ));
        Self {
            engine,
            root,
            count,
            sessions,
            events,
        }
    }

    fn dir(&self, name: &str) -> String {
        let path = self.root.path().join(name);
        fs::create_dir_all(&path).expect("working directory");
        path.to_string_lossy().into_owned()
    }

    fn sessions(&self, dirs: &[String]) -> Vec<Session> {
        dirs.iter()
            .enumerate()
            .map(|(i, dir)| self.engine.create_session(dir, "", &format!("s{i}")))
            .collect()
    }

    fn initialize_count(&self) -> usize {
        fs::read(&self.count).map(|bytes| bytes.len()).unwrap_or(0)
    }

    fn event_count(&self, event: &str) -> usize {
        fs::read_to_string(&self.events)
            .unwrap_or_default()
            .lines()
            .filter(|line| *line == event)
            .count()
    }

    fn session_count(&self) -> usize {
        fs::read_to_string(&self.sessions)
            .ok()
            .and_then(|count| count.parse().ok())
            .unwrap_or(0)
    }
}

const FAKE_AGENT: &str = r###"#!/bin/sh
count="$1"
events="$2"
sessions="$3"
session=0
printf 'spawn\n' >> "$events"
trap 'printf "exit\n" >> "$events"' EXIT
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf x >> "$count"
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true}}}\n' "$id"
      ;;
    *'"method":"session/new"'*)
      session=$((session + 1))
      total=$(cat "$sessions" 2>/dev/null || printf 0)
      printf '%s' "$((total + 1))" > "$sessions"
      printf '{"jsonrpc":"2.0","id":%s,"result":{"sessionId":"fake-%s","configOptions":[]}}\n' "$id" "$session"
      ;;
    *'"method":"session/load"'*)
      printf 'load\n' >> "$events"
      total=$(cat "$sessions" 2>/dev/null || printf 0)
      printf '%s' "$((total + 1))" > "$sessions"
      printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id"
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id"
      ;;
  esac
done
"###;

#[tokio::test]
async fn same_directory_shares_one_process_but_gets_distinct_sessions() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir.clone()]);
    let first = h
        .engine
        .conns
        .ensure(&h.engine, &sessions[0].id)
        .await
        .unwrap();
    let second = h
        .engine
        .conns
        .ensure(&h.engine, &sessions[1].id)
        .await
        .unwrap();
    assert_eq!(h.initialize_count(), 1);
    assert_ne!(first.acp_session, second.acp_session);
    h.engine.conns.close_all().await;
}

#[tokio::test]
async fn a_binding_to_a_removed_entry_reopens_on_the_next_ensure() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir.clone()]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    assert_eq!(h.session_count(), 2);

    // Simulate the close/attach race: the second session remains bound, but is no longer
    // counted on the old entry when the first session's close removes and kills that entry.
    let slot = h
        .engine
        .conns
        .pool
        .lock()
        .await
        .get(&dir)
        .cloned()
        .expect("pool slot");
    let entry = slot.get().cloned().expect("pooled entry");
    entry.lock().await.attached.remove(&sessions[1].id);
    h.engine.conns.close(&sessions[0].id).await;

    h.engine
        .conns
        .ensure(&h.engine, &sessions[1].id)
        .await
        .unwrap();
    assert_eq!(h.initialize_count(), 2);
    assert_eq!(
        h.session_count(),
        3,
        "the stale binding must issue session/load"
    );
    assert_eq!(h.event_count("load"), 1);
    h.engine.conns.close_all().await;
}

#[tokio::test]
async fn different_directories_and_opt_out_do_not_share() {
    let h = Harness::new(true);
    let sessions = h.sessions(&[h.dir("one"), h.dir("two")]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    assert_eq!(h.initialize_count(), 2);
    h.engine.conns.close_all().await;

    let h = Harness::new(false);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    assert_eq!(h.initialize_count(), 2);
    h.engine.conns.close_all().await;
}

#[tokio::test]
async fn closing_one_session_detaches_and_closing_the_last_kills() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    h.engine.conns.close(&sessions[0].id).await;
    assert!(h.engine.conns.existing(&sessions[1].id).await.is_some());
    h.engine.conns.close(&sessions[1].id).await;
    assert!(h.engine.conns.existing(&sessions[1].id).await.is_none());
}

#[tokio::test]
async fn eviction_unbinds_every_session_and_next_use_loads_on_a_new_process() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    let affected = h.engine.conns.evict(&sessions[0].id).await;
    assert_eq!(affected.len(), 1);
    assert!(h.engine.conns.existing(&sessions[0].id).await.is_none());
    assert!(h.engine.conns.existing(&sessions[1].id).await.is_none());
    h.engine
        .conns
        .ensure(&h.engine, &sessions[0].id)
        .await
        .unwrap();
    assert_eq!(h.initialize_count(), 2);
    assert_eq!(h.event_count("load"), 1);
    h.engine.conns.close_all().await;
}

#[tokio::test]
async fn concurrent_first_use_spawns_and_initializes_once() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    let (first, second) = tokio::join!(
        h.engine.conns.ensure(&h.engine, &sessions[0].id),
        h.engine.conns.ensure(&h.engine, &sessions[1].id),
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(h.initialize_count(), 1);
    h.engine.conns.close_all().await;
}

/// The assumption the whole pool rests on: a real agent serves two concurrent sessions on
/// one process. Verified against `claude-code-acp`, whose adapter keys state by session id.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires npx and Claude credentials"]
async fn live_two_sessions_share_one_child_and_answer_independently() {
    use crate::acp_engine::{attach, config, turn};
    use serde_json::Value;

    let cfg = config::load();
    let (_, agent) = cfg.for_runner("claude").expect("claude agent");
    let flags = BuiltinFlags::default();
    let registry = RegistryHandle::new(Arc::new(McpRegistry::builtins(flags)), flags);
    let engine = Arc::new(AcpEngine::new(
        "live-pool".to_string(),
        agent.clone(),
        None,
        registry,
    ));

    let dir = std::env::temp_dir().join("opman-acp-live-pool");
    fs::create_dir_all(&dir).expect("temp dir");
    let dir = dir.to_string_lossy().into_owned();
    let a = engine.create_session(&dir, "", "pool a");
    let b = engine.create_session(&dir, "", "pool b");

    let mut events = engine.subscribe_raw();
    turn::prompt(
        engine.clone(),
        a.id.clone(),
        attach::Prompt::text("Reply with exactly the word ALPHA and nothing else."),
    )
    .await;
    turn::prompt(
        engine.clone(),
        b.id.clone(),
        attach::Prompt::text("Reply with exactly the word BRAVO and nothing else."),
    )
    .await;

    let mut text_a = String::new();
    let mut text_b = String::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(raw)) = tokio::time::timeout_at(deadline, events.recv()).await else {
            break;
        };
        let Ok(event) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let props = &event["properties"];
        let sid = props["sessionID"].as_str().unwrap_or_default();
        if let Some(chunk) = props["part"]["text"].as_str() {
            if sid == a.id {
                text_a.push_str(chunk);
            } else if sid == b.id {
                text_b.push_str(chunk);
            }
        }
        if text_a.contains("ALPHA") && text_b.contains("BRAVO") {
            break;
        }
    }

    let ready_a = engine.conns.ensure(&engine, &a.id).await.expect("a ready");
    let ready_b = engine.conns.ensure(&engine, &b.id).await.expect("b ready");
    assert_ne!(
        ready_a.acp_session, ready_b.acp_session,
        "sessions must be distinct on the shared child"
    );
    assert!(text_a.contains("ALPHA"), "session a said: {text_a:?}");
    assert!(text_b.contains("BRAVO"), "session b said: {text_b:?}");
    engine.conns.close_all().await;
}
