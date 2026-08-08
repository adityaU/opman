//! More live round-trips: a new session driven the way the web layer drives one, and a real
//! image sent to a real agent.
//!
//! Split from [`super::mod_live_tests`] on size alone; the setup and the caveats are the same.
//!
//! Run with: `cargo test --bin opman acp_engine::mod_live_send_tests -- --ignored --nocapture`

use crate::acp_engine::*;

fn test_engine(agent: config::AgentConfig) -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new("test".to_string(), agent, None, registry))
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
