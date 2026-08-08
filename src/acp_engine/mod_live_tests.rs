//! Live round-trips against a real ACP server.
//!
//! Split from [`super::mod_tests`], which needs nothing but the engine struct. Everything
//! here spawns the actual agent, so all of it is `#[ignore]`d: it needs npx, network access
//! on first run, and working credentials.
//!
//! Run with: `cargo test --bin opman acp_engine::mod_live_tests -- --ignored --nocapture`

use crate::acp_engine::*;

fn test_engine(agent: config::AgentConfig) -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new("test".to_string(), agent, None, registry))
}

/// A live round-trip against the real ACP server: spawn it, stream a reply, and assert that
/// text arrived incrementally rather than in one lump. Ignored by default because it needs
/// npx, network access on first run, and working Claude credentials.
///
/// Run with: `cargo test --bin opman acp_engine::mod_tests::live -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires npx and Claude credentials"]
async fn live_prompt_streams_incrementally() {
    let cfg = config::load();
    let (id, agent) = cfg
        .for_runner("claude")
        .expect("built-in claude agent should be configured");
    let engine = test_engine(agent.clone());
    println!("agent `{id}`: {} {:?}", agent.command, agent.args);

    let dir = std::env::temp_dir().join("opman-acp-live");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let session = engine.create_session(&dir.to_string_lossy(), "", "live test");

    let mut events = engine.subscribe_raw();
    turn::prompt(
        engine.clone(),
        session.id.clone(),
        attach::Prompt::text(
            "Run the bash command `echo streaming works`, then count from 1 to 40 \
             in words, one per line, with no preamble.",
        ),
    )
    .await;

    let mut deltas = 0usize;
    let mut text = String::new();
    let mut tools: Vec<(String, String)> = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(raw)) = tokio::time::timeout_at(deadline, events.recv()).await else {
            break;
        };
        let Ok(event) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let props = &event["properties"];
        match event["type"].as_str().unwrap_or_default() {
            "message.part.delta" => {
                deltas += 1;
                text.push_str(props["delta"].as_str().unwrap_or_default());
            }
            "message.part.updated" => {
                let part = &props["part"];
                if part["type"] == "text"
                    && part["messageID"]
                        .as_str()
                        .is_some_and(|m| !m.starts_with("msg_user"))
                {
                    text.push_str(part["text"].as_str().unwrap_or_default());
                }
                if part["type"] == "tool" {
                    tools.push((
                        part["tool"].as_str().unwrap_or_default().to_string(),
                        part["state"]["status"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    ));
                }
            }
            "session.idle" => break,
            _ => {}
        }
    }
    println!("deltas={deltas} tools={tools:?}\n--- text ---\n{text}\n---");
    assert!(
        text.to_lowercase().contains("forty"),
        "expected the counted reply, got {text:?}"
    );
    // The point of the rewrite: the reply arrives incrementally, as a first part followed by
    // deltas, rather than materialising whole at the end of the turn. How *fine* the chunks
    // are is the agent's choice — Claude's adapter batches a long reply into a handful of
    // sizeable chunks — so this asserts that opman forwards each chunk as it arrives without
    // coalescing, not a token count opman does not control.
    assert!(deltas > 0, "reply did not stream incrementally");
    // And the agent's real tool name must survive translation, ending in a settled state.
    assert!(
        tools.iter().any(|(name, _)| name == "Bash"),
        "expected a Bash tool part, got {tools:?}"
    );
    assert!(
        tools.iter().any(|(_, status)| status == "completed"),
        "expected the tool call to complete, got {tools:?}"
    );
}

/// The engine picker asks for models before any session exists, so the startup probe must
/// fill the catalogue in. Without it `/provider` is empty until the user's first message —
/// which is exactly the "models are not coming" symptom.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires npx and Claude credentials"]
async fn live_capability_probe_populates_models_and_modes() {
    let cfg = config::load();
    let (id, agent) = cfg
        .for_runner("claude")
        .expect("built-in claude agent should be configured");
    let engine = test_engine(agent.clone());

    assert!(
        engine.models().is_empty(),
        "no catalogue should exist before the probe"
    );
    let setup = conn::probe_capabilities(&engine)
        .await
        .expect("capability probe should succeed");
    engine.set_capabilities(setup);

    let models = engine.models();
    let modes = engine.modes();
    println!(
        "agent `{id}` models={:?} modes={:?}",
        models.iter().map(|m| &m.id).collect::<Vec<_>>(),
        modes.iter().map(|m| &m.id).collect::<Vec<_>>()
    );
    assert!(!models.is_empty(), "probe reported no models");
    assert!(!modes.is_empty(), "probe reported no permission modes");

    // And the picker payload must carry them in the shape it actually reads.
    let payload = options::provider_payload(
        &engine.id,
        &engine.agent.display_name,
        &models,
        engine.current_model().as_deref(),
        &modes,
    );
    let listed = payload["all"][0]["models"]
        .as_object()
        .expect("models map in the provider payload");
    assert!(!listed.is_empty(), "provider payload exposed no models");
    assert!(!payload["permissionModes"]
        .as_array()
        .unwrap_or(&vec![])
        .is_empty());
}
