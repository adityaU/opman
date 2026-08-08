//! The stdio half's framing.
//!
//! [`super::bridge_tests`] covers translating a tool call into a manager request. This
//! covers the loop around it: what a runner reads back for each JSON-RPC method, and what
//! happens to the line that was not JSON at all.

use super::*;

/// Drive the bridge over a pipe and collect every line it writes.
///
/// The socket path is deliberately one nothing listens on: these assertions are about the
/// protocol, and a `tools/call` that cannot reach the manager still has to come back as a
/// well-formed tool error rather than silence.
async fn converse(requests: &[Value]) -> Vec<Value> {
    let input: String = requests
        .iter()
        .map(|request| format!("{request}\n"))
        .collect();
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    run_bridge_over(
        input.as_bytes(),
        output.clone(),
        Arc::new(std::env::temp_dir().join("opman-bridge-tests-absent.sock")),
        Some("ses_parent".to_string()),
        std::path::PathBuf::from("/work"),
    )
    .await
    .expect("the bridge should run to end of input");
    let written = output.lock().await;
    String::from_utf8_lossy(&written)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[tokio::test]
async fn initialize_advertises_tools_and_names_the_server() {
    let replies = converse(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" })]).await;

    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        "opman-agent-manager"
    );
    assert!(replies[0]["result"]["capabilities"]["tools"].is_object());
}

/// The seven tools are the contract an agent reads before it calls anything.
#[tokio::test]
async fn listing_returns_every_tool_the_manager_implements() {
    let replies = converse(&[json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })]).await;

    let names: Vec<&str> = replies[0]["result"]["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    for expected in [
        "agent_send",
        "agent_start",
        "agent_progress",
        "agent_runner_options",
        "agent_list",
        "agent_wait",
        "agent_abort",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
}

/// A notification has no id and must draw no reply, or the runner sees a response to a
/// request it never made.
#[tokio::test]
async fn the_initialized_notification_is_answered_with_silence() {
    let replies = converse(&[
        json!({ "jsonrpc": "2.0", "id": 0, "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
    ])
    .await;

    assert_eq!(replies.len(), 1, "only the initialize should answer");
    assert_eq!(replies[0]["id"], 1);
}

#[tokio::test]
async fn an_unknown_method_is_a_protocol_error_naming_the_method() {
    let replies =
        converse(&[json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/teleport" })]).await;

    assert_eq!(replies[0]["error"]["code"], -32601);
    assert!(
        replies[0]["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("tools/teleport"),
        "{:?}",
        replies[0]
    );
}

/// A malformed line must not end the session: the runner would lose every later call on
/// the same pipe.
#[tokio::test]
async fn a_line_that_is_not_json_is_a_parse_error_and_the_loop_continues() {
    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    run_bridge_over(
        "{not json\n\n{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"initialize\"}\n".as_bytes(),
        output.clone(),
        Arc::new(std::path::PathBuf::from("/nowhere.sock")),
        None,
        std::path::PathBuf::from("/work"),
    )
    .await
    .expect("the bridge survives a bad line");

    let written = output.lock().await;
    let replies: Vec<Value> = String::from_utf8_lossy(&written)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[0]["id"], Value::Null);
    assert_eq!(replies[1]["id"], 4, "later requests still answer");
}

/// An unreachable manager is reported as a tool error, not as a JSON-RPC error: the call
/// itself was well-formed, and the agent should read the reason in its transcript.
#[tokio::test]
async fn a_tool_call_that_cannot_reach_the_manager_comes_back_as_a_tool_error() {
    let replies = converse(&[json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": "agent_list", "arguments": {} },
    })])
    .await;

    assert_eq!(replies[0]["id"], 7);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert!(
        replies[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("failed to connect"),
        "{:?}",
        replies[0]
    );
}

/// The one that produced a tool call that never returned.
///
/// A `tools/call` runs in a spawned task so a slow steer cannot stall the pipe. When the
/// input then ends, returning from the loop drops those tasks — and with them the response
/// the caller is blocked on. Nothing is logged and nothing fails; the calling agent simply
/// sits inside a tool forever. Every call must have answered before the loop returns.
///
/// `converse` does not sleep, so this assertion only holds because the drain does.
#[tokio::test]
async fn a_call_still_running_when_the_input_ends_is_answered_before_the_loop_returns() {
    let replies = converse(&[json!({
        "jsonrpc": "2.0", "id": 21, "method": "tools/call",
        "params": { "name": "agent_send", "arguments": {
            "message": "hi", "model": "haiku", "effort": "low",
        }},
    })])
    .await;

    assert_eq!(
        replies.len(),
        1,
        "the in-flight call was dropped: {replies:?}"
    );
    assert_eq!(replies[0]["id"], 21);
}

/// Calls are spawned rather than awaited in turn, so a slow one must not stop the ones
/// behind it — and every reply must still be a whole line.
#[tokio::test]
async fn concurrent_calls_each_answer_on_their_own_id() {
    let replies = converse(&[
        json!({ "jsonrpc": "2.0", "id": 11, "method": "tools/call",
                "params": { "name": "agent_list", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 12, "method": "tools/call",
                "params": { "name": "agent_progress", "arguments": {} } }),
    ])
    .await;

    let mut ids: Vec<i64> = replies
        .iter()
        .filter_map(|reply| reply["id"].as_i64())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![11, 12]);
}
