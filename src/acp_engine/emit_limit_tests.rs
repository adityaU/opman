//! A turn that fails on a spent plan carries a structured `limit` on its system error.

use super::*;

fn engine() -> (Arc<AcpEngine>, String) {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    let agent = config::AgentConfig::default();
    let engine = Arc::new(AcpEngine::new("test".to_string(), agent, None, registry));
    let session = engine.create_session("/tmp", "", "limits");
    (engine, session.id)
}

/// The `info` of the first `message.updated` emitted for `text` at `level`.
fn emitted_info(level: &str, text: &str) -> Value {
    let (engine, id) = engine();
    let mut rx = engine.subscribe();
    emit_system(&engine, &id, level, text);
    while let Ok(event) = rx.try_recv() {
        let Ok(frame) = serde_json::from_str::<Value>(&event.data) else {
            continue;
        };
        if frame["type"] == "message.updated" {
            return frame["properties"]["info"].clone();
        }
    }
    Value::Null
}

#[test]
fn a_usage_limit_error_is_tagged() {
    let info = emitted_info("error", "Internal error: Claude AI usage limit reached|1760000000");
    assert_eq!(info["variant"], "error");
    assert_eq!(info["limit"]["kind"], "usage");
    assert_eq!(info["limit"]["resetsAt"], "2025-10-09T08:53:20Z");
}

#[test]
fn a_codex_limit_error_is_tagged() {
    let info = emitted_info("error", "You've hit your usage limit. Try again at 3:05 PM.");
    assert_eq!(info["limit"]["kind"], "usage");
}

#[test]
fn other_errors_carry_no_limit() {
    let info = emitted_info("error", "acp process exited");
    assert_eq!(info["variant"], "error");
    assert!(info.get("limit").is_none());
}

#[test]
fn warnings_are_never_tagged() {
    // A warning that merely mentions a limit is advice, not a failed turn.
    let info = emitted_info("warning", "You are close to your usage limit reached threshold");
    assert!(info.get("limit").is_none());
}
