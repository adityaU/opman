//! What one `session/update` does to the rendered conversation.

use super::*;

use crate::acp_engine::config::{AgentConfig, ClientCaps};
use crate::acp_engine::terminal;

/// An engine with one session, bound to the ACP session id these updates arrive under.
fn engine() -> (Arc<AcpEngine>, String) {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    let agent = AgentConfig {
        client_caps: ClientCaps {
            terminal: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = Arc::new(AcpEngine::new("test".to_string(), agent, None, registry));
    let session = engine.create_session("/tmp", "", "render");
    engine.bind_acp_session(&session.id, "acp-1");
    (engine, session.id)
}

/// Every part of the live assistant message, in order.
fn parts(engine: &Arc<AcpEngine>, session_id: &str) -> Vec<Value> {
    engine.with_transcript(session_id, |t| {
        t.messages()
            .iter()
            .flat_map(|m| m.parts.iter().cloned())
            .collect()
    })
}

#[test]
fn text_chunks_stream_into_one_part() {
    let (engine, id) = engine();
    for text in ["Hel", "lo"] {
        apply(
            &engine,
            &id,
            &json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } }),
        );
    }
    let parts = parts(&engine, &id);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["text"], "Hello");
}

/// The gap: an agent answering with an image used to produce an empty message, because only
/// `content.text` was read. It becomes a `file` part — the same shape the timeline already
/// renders for a user's attachments.
#[test]
fn an_image_from_the_agent_becomes_a_file_part() {
    let (engine, id) = engine();
    apply(
        &engine,
        &id,
        &json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "image", "mimeType": "image/png", "data": "AAAA" },
        }),
    );
    let parts = parts(&engine, &id);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["type"], "file");
    assert_eq!(parts[0]["mime"], "image/png");
    assert_eq!(parts[0]["url"], "data:image/png;base64,AAAA");
}

/// Prose on either side of a non-prose block stays separate: an image has no meaningful
/// "append", so the streaming run ends at it and restarts after.
#[test]
fn text_around_an_image_lands_in_separate_parts() {
    let (engine, id) = engine();
    let chunk =
        |content: Value| json!({ "sessionUpdate": "agent_message_chunk", "content": content });
    apply(
        &engine,
        &id,
        &chunk(json!({ "type": "text", "text": "before" })),
    );
    apply(
        &engine,
        &id,
        &chunk(json!({ "type": "image", "mimeType": "image/png", "data": "AAAA" })),
    );
    apply(
        &engine,
        &id,
        &chunk(json!({ "type": "text", "text": "after" })),
    );

    let parts = parts(&engine, &id);
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0]["text"], "before");
    assert_eq!(parts[1]["type"], "file");
    assert_eq!(parts[2]["text"], "after");
}

/// A cited file is a link, which keeps the path clickable rather than dropping it.
#[test]
fn a_resource_link_renders_as_a_link() {
    let (engine, id) = engine();
    apply(
        &engine,
        &id,
        &json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "resource_link", "uri": "file:///a/b.rs", "name": "b.rs" },
        }),
    );
    assert_eq!(parts(&engine, &id)[0]["text"], "[b.rs](file:///a/b.rs)");
}

/// ACP lets a tool call point at a terminal instead of carrying its output. Without this the
/// card renders empty, because the block has no content of its own to read.
#[tokio::test]
async fn tool_content_pointing_at_a_terminal_shows_that_terminal_output() {
    let (engine, id) = engine();
    let created = terminal::create(
        &engine,
        &json!({ "sessionId": "acp-1", "command": "echo", "args": ["from the terminal"] }),
    )
    .await
    .expect("terminal/create");
    let terminal_id = created["terminalId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let _ = terminal::wait_for_exit(
        &engine,
        &json!({ "sessionId": "acp-1", "terminalId": terminal_id }),
    )
    .await;

    apply(
        &engine,
        &id,
        &json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "call-1",
            "title": "bash",
            "status": "completed",
            "content": [{ "type": "terminal", "terminalId": terminal_id }],
        }),
    );

    let parts = parts(&engine, &id);
    assert_eq!(parts[0]["type"], "tool");
    assert_eq!(parts[0]["state"]["output"], "from the terminal\n");
}

/// A tool call carrying its own content is untouched — the terminal path must not cost the
/// common case a rewrite.
#[test]
fn ordinary_tool_content_is_left_alone() {
    let (engine, id) = engine();
    apply(
        &engine,
        &id,
        &json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "call-1",
            "title": "bash",
            "content": [{ "type": "content", "content": { "type": "text", "text": "plain" } }],
        }),
    );
    assert_eq!(parts(&engine, &id)[0]["state"]["output"], "plain");
}

/// The protocol is versioned and additive: an update opman has never heard of is ignored
/// rather than treated as an error.
#[test]
fn unknown_update_kinds_are_ignored() {
    let (engine, id) = engine();
    apply(&engine, &id, &json!({ "sessionUpdate": "telepathy" }));
    assert!(parts(&engine, &id).is_empty());
}
