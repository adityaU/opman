//! Additional dispatch coverage for the MCP WebSocket handler.
//!
//! `handler_tests.rs` covers the core dispatch arms; this file fills the gaps:
//! the `McpAgentActivity` active/inactive event pair emitted around every
//! `tools/call`, the editor tool arms, and a real-PTY terminal tool-call through
//! `dispatch_method`.
//!
//! `websocket_handler` and `handle_mcp_session` are NOT exercised: the former
//! requires a real WebSocket upgrade plus auth, and the latter is an infinite
//! read loop over a live `WebSocket` that cannot be constructed in a unit test.
//! Their JSON-RPC framing branches (PARSE_ERROR on bad JSON, INVALID_REQUEST on
//! a wrong `jsonrpc` version) live inside that loop and are unreachable here.

use super::*;
use crate::web::test_support::test_server_state;
use crate::web::types::{ServerState, WebEvent};

fn req(v: serde_json::Value) -> JsonRpcRequest {
    serde_json::from_value(v).unwrap()
}

fn to_value(resp: &JsonRpcResponse) -> serde_json::Value {
    serde_json::to_value(resp).unwrap()
}

#[tokio::test]
async fn tools_call_emits_agent_activity_on_and_off() {
    let state = test_server_state();
    let mut rx = state.event_tx.subscribe();
    let _ = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"web_terminal_list","arguments":{}}
        })),
    )
    .await;

    let mut on = false;
    let mut off = false;
    while let Ok(ev) = rx.try_recv() {
        if let WebEvent::McpAgentActivity { tool, active } = ev {
            if tool == "web_terminal_list" {
                if active {
                    on = true;
                } else {
                    off = true;
                }
            }
        }
    }
    assert!(on && off, "expected both active=true and active=false activity events");
}

#[tokio::test]
async fn tools_call_editor_open_missing_args_is_error_content() {
    let state = test_server_state();
    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":"web_editor_open","arguments":{}}
        })),
    )
    .await;
    let v = to_value(&r);
    // Missing required args → handler errors → wrapped as isError content.
    assert_eq!(v["result"]["isError"], true);
    assert!(v["result"]["content"][0]["text"].as_str().is_some());
}

#[tokio::test]
async fn tools_call_editor_read_and_list_return_content() {
    let state = test_server_state();
    for name in ["web_editor_read", "web_editor_list"] {
        let r = dispatch_method(
            &state,
            &req(serde_json::json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name": name, "arguments":{}}
            })),
        )
        .await;
        let v = to_value(&r);
        // Whether ok or error, dispatch always produces a text content block.
        assert!(v["result"]["content"][0]["text"].as_str().is_some(), "{name} produced no content");
    }
}

#[tokio::test]
async fn tools_call_terminal_run_and_close_error_arms() {
    // Against the no-op pty handle these arms take their error branch, but they
    // still route through the tool-name match in `handle_tools_call`.
    let state = test_server_state();
    for name in ["web_terminal_run", "web_terminal_new", "web_terminal_close", "web_terminal_read"] {
        let r = dispatch_method(
            &state,
            &req(serde_json::json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name": name, "arguments":{"id":"x","command":"ls"}}
            })),
        )
        .await;
        let v = to_value(&r);
        assert_eq!(v["result"]["isError"], true, "{name} should have errored on no-op pty");
    }
}

/// Drive a real terminal tool-call success through `dispatch_method` so the
/// `web_terminal_new` arm's Ok branch is covered end-to-end.
#[tokio::test]
async fn tools_call_terminal_new_success_via_dispatch() {
    let mut state: ServerState = test_server_state();
    state.pty_mgr = crate::web::pty_manager::start_web_pty_manager();

    let r = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":5,"method":"tools/call",
            "params":{"name":"web_terminal_new","arguments":{"rows":24,"cols":80}}
        })),
    )
    .await;
    let v = to_value(&r);
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    if v["result"].get("isError").is_some() {
        return; // environment can't spawn a PTY — nothing to clean up
    }
    let inner: serde_json::Value = serde_json::from_str(text).unwrap();
    let id = inner["id"].as_str().unwrap();
    assert!(!id.is_empty());

    // Close it via dispatch too, covering the terminal_close Ok arm.
    let rc = dispatch_method(
        &state,
        &req(serde_json::json!({
            "jsonrpc":"2.0","id":6,"method":"tools/call",
            "params":{"name":"web_terminal_close","arguments":{"id": id}}
        })),
    )
    .await;
    let vc = to_value(&rc);
    assert!(vc["result"]["content"][0]["text"].as_str().unwrap().contains("closed"));
}
