//! Turning a `tools/call` into a manager request. The bridge forwards; it does not judge.

use super::*;

fn request(name: &str, args: serde_json::Value) -> Result<ManagerRequest> {
    to_request(name, &args, Some("ses_parent"), "/work")
}

#[test]
fn the_four_tools_map_to_the_four_operations() {
    for (tool, op) in [
        ("agent_send", "send"),
        ("agent_start", "start"),
        ("agent_progress", "progress"),
        ("agent_runner_options", "options"),
    ] {
        let parsed = request(tool, json!({})).expect("a known tool");
        assert_eq!(parsed.op, op, "{tool}");
    }
}

#[test]
fn an_unknown_tool_is_named_in_the_error() {
    let error = request("agent_teleport", json!({})).expect_err("unknown tool");

    assert!(format!("{error}").contains("agent_teleport"), "{error}");
}

/// Both halves must survive the hop, or the manager would refuse a call the agent made
/// correctly.
#[test]
fn the_model_and_effort_are_forwarded_verbatim() {
    let parsed = request(
        "agent_send",
        json!({ "message": "go", "model": "claude-opus-5", "effort": "xhigh", "provider": "anthropic" }),
    )
    .expect("valid");

    assert_eq!(parsed.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(parsed.effort.as_deref(), Some("xhigh"));
    assert_eq!(parsed.provider.as_deref(), Some("anthropic"));
}

/// The bridge does not enforce the contract — the manager does, once, for both the socket
/// and the stdio callers. Rejecting here as well would be a second copy to keep in step.
#[test]
fn a_call_missing_the_required_halves_is_still_forwarded() {
    let parsed = request("agent_send", json!({ "message": "go" })).expect("forwarded");

    assert!(parsed.model.is_none());
    assert!(parsed.effort.is_none());
    assert!(parsed.dispatch().is_err(), "the manager is what refuses it");
}

/// Two tools, two names for the same argument: `to` on send, `agent_id` on progress.
#[test]
fn either_spelling_of_the_target_is_accepted() {
    let sent = request("agent_send", json!({ "to": "ses_a" })).expect("valid");
    assert_eq!(sent.target.as_deref(), Some("ses_a"));

    let asked = request("agent_progress", json!({ "agent_id": "ses_b" })).expect("valid");
    assert_eq!(asked.target.as_deref(), Some("ses_b"));
}

#[test]
fn the_calling_session_and_project_directory_are_attached() {
    let parsed = request("agent_progress", json!({})).expect("valid");

    assert_eq!(parsed.source_session.as_deref(), Some("ses_parent"));
    assert_eq!(parsed.directory.as_deref(), Some("/work"));
}

/// `tools/call` with no `arguments` at all is legal JSON-RPC, and the two read-only tools
/// are routinely called that way.
#[tokio::test]
async fn a_call_without_arguments_does_not_fail_before_it_is_sent() {
    let missing = std::env::temp_dir().join("opman-agent-manager-absent.sock");

    let error = call_tool(
        &missing,
        Some(json!({ "name": "agent_runner_options" })),
        None,
        "/work",
    )
    .await
    .expect_err("there is no manager listening");

    // It got as far as the socket, which is the point: the arguments parsed.
    assert!(
        format!("{error:#}").contains("failed to connect"),
        "{error:#}"
    );
}

#[tokio::test]
async fn a_call_without_a_tool_name_is_refused() {
    let error = call_tool(Path::new("/nowhere"), Some(json!({})), None, "/work")
        .await
        .expect_err("a call needs a name");

    assert!(format!("{error}").contains("missing tool name"), "{error}");
}
