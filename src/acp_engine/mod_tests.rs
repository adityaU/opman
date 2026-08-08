//! Engine-level tests, including a live round-trip against a real ACP server.

use super::*;

fn test_engine(agent: config::AgentConfig) -> Arc<AcpEngine> {
    engine_with_mcp(agent, crate::mcp_registry::BuiltinFlags::default())
}

fn engine_with_mcp(
    agent: config::AgentConfig,
    flags: crate::mcp_registry::BuiltinFlags,
) -> Arc<AcpEngine> {
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new("test".to_string(), agent, None, registry))
}

/// ACP reports per-turn usage; the engine this replaced had no channel for it, which is why
/// its sessions always displayed zero tokens.
#[test]
fn usage_tokens_maps_acp_field_names() {
    let tokens = usage_tokens(&json!({
        "inputTokens": 4, "outputTokens": 295,
        "cachedReadTokens": 78535, "cachedWriteTokens": 5807, "totalTokens": 84641
    }));
    assert_eq!(tokens["input"], 4);
    assert_eq!(tokens["output"], 295);
    assert_eq!(tokens["cache"]["read"], 78535);
    assert_eq!(tokens["cache"]["write"], 5807);
}

/// Missing fields must read as zero rather than dropping the whole usage report: agents are
/// free to omit any of these.
#[test]
fn usage_tokens_defaults_absent_fields_to_zero() {
    let tokens = usage_tokens(&json!({ "outputTokens": 12 }));
    assert_eq!(tokens["input"], 0);
    assert_eq!(tokens["output"], 12);
    assert_eq!(tokens["cache"]["read"], 0);
}

/// MCP injection is opt-out per agent, because an agent that cannot speak MCP should not be
/// handed a server list it will choke on.
#[test]
fn mcp_servers_are_omitted_when_injection_is_disabled() {
    let agent = config::AgentConfig {
        inject_mcp: false,
        ..Default::default()
    };
    let engine = test_engine(agent);
    assert_eq!(
        engine.mcp_servers("/tmp", "ses1", mcp_servers::McpCaps::default()),
        json!([])
    );
}

/// ACP wants `mcpServers` as a list of named stdio servers with `env` as name/value pairs —
/// a different shape from the `--mcp-config` object the CLI took.
#[test]
fn mcp_servers_use_the_acp_list_shape() {
    let agent = config::AgentConfig {
        inject_mcp: true,
        ..Default::default()
    };
    let flags = crate::mcp_registry::BuiltinFlags {
        terminal: true,
        time: true,
        ..Default::default()
    };
    let engine = engine_with_mcp(agent, flags);
    let servers = engine.mcp_servers("/tmp/project", "ses1", mcp_servers::McpCaps::default());
    let list = servers.as_array().expect("expected a list of servers");

    let time = list
        .iter()
        .find(|s| s["name"] == "time")
        .expect("time server should be present");
    assert_eq!(time["args"][0], "mcp-time");

    // env is a name/value pair list, not an object. Only the bridges that route by
    // session declare it — `time` does not read it, so it no longer carries one.
    let terminal = list
        .iter()
        .find(|s| s["name"] == "terminal")
        .expect("terminal server should be present");
    assert_eq!(terminal["args"][1], "/tmp/project");
    assert_eq!(terminal["env"][0]["name"], "OPENCODE_SESSION_ID");
    assert_eq!(terminal["env"][0]["value"], "ses1");
    assert_eq!(time["env"], json!([]));
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

/// Drive a brand-new session the way the web layer does — `POST /session` then
/// `POST /session/{id}/message` — and record every event. Reproduces the reported
/// new-session failure: a duplicated prompt and "No response".
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires npx and Claude credentials"]
async fn live_first_send_emits_one_user_message_and_a_reply() {
    let cfg = config::load();
    let (_, agent) = cfg
        .for_runner("claude")
        .expect("built-in claude agent should be configured");
    let engine = test_engine(agent.clone());

    let dir = std::env::temp_dir().join("opman-acp-firstsend");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let session = engine.create_session(&dir.to_string_lossy(), "", "New session");

    let mut events = engine.subscribe_raw();
    // Exactly what the composer posts: text parts plus the picker's selections.
    let body = json!({
        "parts": [{ "type": "text", "text": "Reply with the single word: pong" }],
        "permission": "bypassPermissions",
    });
    let engine_for_send = engine.clone();
    let sid = session.id.clone();
    tokio::spawn(async move {
        turn::prompt(engine_for_send, sid, attach::Prompt::from_body(&body)).await;
    });

    let mut user_messages: Vec<String> = Vec::new();
    let mut assistant_text = String::new();
    let mut saw_assistant_envelope = false;
    let mut system_bubbles: Vec<String> = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(150);
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(raw)) = tokio::time::timeout_at(deadline, events.recv()).await else {
            break;
        };
        let Ok(event) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let props = &event["properties"];
        match event["type"].as_str().unwrap_or_default() {
            "message.updated" => {
                let info = &props["info"];
                match info["role"].as_str().unwrap_or_default() {
                    "user" => user_messages.push(info["id"].as_str().unwrap_or("").to_string()),
                    "assistant" => saw_assistant_envelope = true,
                    _ => {}
                }
            }
            "message.part.delta" => assistant_text.push_str(props["delta"].as_str().unwrap_or("")),
            "message.part.updated" => {
                let part = &props["part"];
                let mid = part["messageID"].as_str().unwrap_or("");
                if part["type"] == "text" && mid.starts_with("msg_sys") {
                    system_bubbles.push(part["text"].as_str().unwrap_or("").to_string());
                } else if part["type"] == "text" && !mid.starts_with("msg_user") {
                    assistant_text.push_str(part["text"].as_str().unwrap_or(""));
                }
            }
            "session.idle" => break,
            _ => {}
        }
        if std::env::var("ACP_DUMP").is_ok() {
            println!("EVENT {raw}");
        }
    }
    println!(
        "user_messages={user_messages:?}\nassistant_envelope={saw_assistant_envelope}\nsystem={system_bubbles:?}\nassistant_text={assistant_text:?}"
    );
    assert_eq!(
        user_messages.len(),
        1,
        "the engine must emit the prompt exactly once; a second copy is the duplicate bubble"
    );
    assert!(saw_assistant_envelope, "no assistant message was emitted");
    assert!(
        assistant_text.to_lowercase().contains("pong"),
        "expected the model's reply, got {assistant_text:?} (system: {system_bubbles:?})"
    );
}

/// A real image, sent to a real agent, and read back correctly.
///
/// The two halves of upload support fail in different ways, so both are asserted: the agent
/// must actually *see* the picture (it cannot name the colour otherwise), and the user's own
/// bubble must carry a `file` part the timeline can render as a preview.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires npx and Claude credentials"]
async fn live_image_attachment_reaches_the_agent_and_renders() {
    let cfg = config::load();
    let (_, agent) = cfg
        .for_runner("claude")
        .expect("built-in claude agent should be configured");
    let engine = test_engine(agent.clone());

    let dir = std::env::temp_dir().join("opman-acp-upload");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let session = engine.create_session(&dir.to_string_lossy(), "", "upload test");

    // A 2x2 solid red PNG. Inlined so the test carries no binary fixture.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4z8AARAwQCgAf7gP9i18U1AAAAABJRU5ErkJggg==";
    let body = json!({
        "parts": [
            { "type": "text", "text": "What single colour fills this image? Answer with one word." },
            {
                "type": "file",
                "mime": "image/png",
                "filename": "red.png",
                "url": format!("data:image/png;base64,{png}"),
            },
        ],
        "permission": "bypassPermissions",
    });

    let mut events = engine.subscribe_raw();
    let sending = engine.clone();
    let sid = session.id.clone();
    tokio::spawn(async move {
        turn::prompt(sending, sid, attach::Prompt::from_body(&body)).await;
    });

    let mut reply = String::new();
    let mut file_parts: Vec<Value> = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(150);
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(raw)) = tokio::time::timeout_at(deadline, events.recv()).await else {
            break;
        };
        let Ok(event) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let props = &event["properties"];
        match event["type"].as_str().unwrap_or_default() {
            "message.part.delta" => reply.push_str(props["delta"].as_str().unwrap_or("")),
            "message.part.updated" => {
                let part = &props["part"];
                if part["type"] == "file" {
                    file_parts.push(part.clone());
                } else if part["type"] == "text"
                    && part["messageID"]
                        .as_str()
                        .is_some_and(|m| !m.starts_with("msg_user"))
                {
                    reply.push_str(part["text"].as_str().unwrap_or(""));
                }
            }
            "session.idle" => break,
            _ => {}
        }
    }
    println!("reply={reply:?} file_parts={file_parts:?}");

    // The preview half: the user's bubble carries the image as a data URL.
    assert_eq!(file_parts.len(), 1, "expected one file part on the prompt");
    assert_eq!(file_parts[0]["mime"], "image/png");
    assert_eq!(file_parts[0]["filename"], "red.png");
    assert!(file_parts[0]["url"]
        .as_str()
        .is_some_and(|u| u.starts_with("data:image/png;base64,")));
    assert!(file_parts[0]["messageID"]
        .as_str()
        .is_some_and(|m| m.starts_with("msg_user")));

    // The upload half: only a model that received the pixels can name the colour.
    assert!(
        reply.to_lowercase().contains("red"),
        "the agent did not see the image; reply was {reply:?}"
    );
}
