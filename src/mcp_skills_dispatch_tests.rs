//! Wave-3 breadth coverage for `mcp_skills.rs`: response/error serde with
//! `skip_serializing_if`, `dispatch_tool` edge branches, `mcp_handler`
//! tools/call with absent params + unknown tool, `get_skills_dir`, and a
//! smoke spawn of the reload server.
use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

fn registry_with(skills: Vec<Skill>) -> SkillsRegistry {
    let mut map = HashMap::new();
    for s in skills {
        map.insert(s.name.clone(), s);
    }
    Arc::new(RwLock::new(map))
}

fn skill(name: &str) -> Skill {
    Skill {
        name: name.to_string(),
        description: format!("desc {name}"),
        content: format!("content {name}"),
    }
}

// ── McpResponse / McpError serde (skip_serializing_if) ───────────────────────

#[test]
fn response_with_result_omits_error_field() {
    let resp = McpResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(json!({ "ok": true })),
        error: None,
        id: json!(1),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["result"]["ok"], true);
    assert!(v.get("error").is_none(), "error should be omitted");
}

#[test]
fn response_with_error_omits_result_field() {
    let resp = McpResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(McpError { code: -32601, message: "nope".to_string() }),
        id: json!(null),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v.get("result").is_none(), "result should be omitted");
    assert_eq!(v["error"]["code"], -32601);
    assert_eq!(v["error"]["message"], "nope");
}

#[test]
fn mcp_request_deserializes_from_wire() {
    let r: McpRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","method":"tools/call","params":{"a":1},"id":3}"#)
            .unwrap();
    assert_eq!(r.method, "tools/call");
    assert!(r.params.is_some());
    assert_eq!(r.id, json!(3));
}

// ── dispatch_tool edge branches ──────────────────────────────────────────────

#[tokio::test]
async fn dispatch_list_skills_empty_registry() {
    let reg = registry_with(vec![]);
    let out = dispatch_tool(&reg, "list_skills", &Value::Null).await;
    // Empty registry serializes to an empty JSON array.
    assert_eq!(out[0]["text"], "[]");
}

#[tokio::test]
async fn dispatch_list_skills_multiple() {
    let reg = registry_with(vec![skill("a"), skill("b")]);
    let out = dispatch_tool(&reg, "list_skills", &Value::Null).await;
    let text = out[0]["text"].as_str().unwrap();
    assert!(text.contains("\"name\""));
    assert!(text.contains("desc a") || text.contains("desc b"));
}

#[tokio::test]
async fn dispatch_load_skill_name_not_string_is_missing() {
    let reg = registry_with(vec![skill("z")]);
    // name present but not a string → as_str() None → "Missing 'name'".
    let out = dispatch_tool(&reg, "load_skill", &json!({ "name": 7 })).await;
    assert!(out[0]["text"].as_str().unwrap().contains("Missing 'name'"));
}

#[tokio::test]
async fn dispatch_load_skill_found_returns_content() {
    let reg = registry_with(vec![skill("found")]);
    let out = dispatch_tool(&reg, "load_skill", &json!({ "name": "found" })).await;
    assert_eq!(out[0]["text"], "content found");
}

#[tokio::test]
async fn dispatch_load_skill_not_found_message() {
    let reg = registry_with(vec![skill("other")]);
    let out = dispatch_tool(&reg, "load_skill", &json!({ "name": "ghost" })).await;
    assert!(out[0]["text"].as_str().unwrap().contains("Skill 'ghost' not found"));
}

#[tokio::test]
async fn dispatch_unknown_tool_message() {
    let reg = registry_with(vec![]);
    let out = dispatch_tool(&reg, "explode", &Value::Null).await;
    assert!(out[0]["text"].as_str().unwrap().contains("Unknown tool: explode"));
}

// ── mcp_handler via State + Json ─────────────────────────────────────────────

async fn call_handler(state: crate::web::types::ServerState, body: Value) -> Value {
    use axum::extract::State;
    use axum::Json;
    let req: McpRequest = serde_json::from_value(body).unwrap();
    let resp = mcp_handler(State(state), Json(req)).await.unwrap();
    serde_json::to_value(&resp.0).unwrap()
}

#[tokio::test]
async fn handler_tools_call_without_params_defaults_null() {
    // No "params" → req.params is None → unwrap_or(Null) → empty tool name →
    // dispatch_tool "_" arm ("Unknown tool: ").
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        json!({ "jsonrpc": "2.0", "method": "tools/call", "id": 7 }),
    )
    .await;
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Unknown tool"), "got: {text}");
}

#[tokio::test]
async fn handler_tools_call_unknown_tool() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        json!({
            "jsonrpc": "2.0", "method": "tools/call", "id": 8,
            "params": { "name": "frob", "arguments": {} }
        }),
    )
    .await;
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool: frob"));
}

#[tokio::test]
async fn handler_tools_call_load_skill_not_found() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        json!({
            "jsonrpc": "2.0", "method": "tools/call", "id": 9,
            "params": { "name": "load_skill", "arguments": { "name": "absent" } }
        }),
    )
    .await;
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn handler_tools_list_schema_shapes() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(state, json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 2 })).await;
    let tools = v["result"]["tools"].as_array().unwrap();
    let load = tools.iter().find(|t| t["name"] == "load_skill").unwrap();
    let req: Vec<&str> = load["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(req, vec!["name"]);
}

// ── get_skills_dir ───────────────────────────────────────────────────────────

#[test]
fn get_skills_dir_ends_with_opman_skills() {
    let dir = get_skills_dir();
    let s = dir.to_string_lossy();
    assert!(s.contains("opman"));
    assert!(dir.ends_with("skills"));
}

// ── spawn_mcp_skills_server smoke ────────────────────────────────────────────

#[tokio::test]
async fn spawn_reload_server_does_not_panic() {
    // Keep the sender alive so the spawned task parks on recv() (it never
    // triggers a real load_skills() against the user's config dir). We only
    // assert the spawn wiring runs without panicking.
    let (tx, rx) = broadcast::channel::<()>(4);
    let reg = registry_with(vec![]);
    spawn_mcp_skills_server(rx, reg);
    // Give the task a scheduling tick to start awaiting.
    tokio::task::yield_now().await;
    drop(tx);
}
