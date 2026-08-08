//! The degraded mode, which is what a runner sees before anyone has logged in.

use super::*;

fn name() -> ServerName {
    ServerName::parse("linear").expect("valid")
}

async fn drive(reason: DegradedReason, lines: &[&str]) -> Vec<Value> {
    let input = lines.join("\n") + "\n";
    let mut output: Vec<u8> = Vec::new();
    run_proxy_over(
        name(),
        Mode::Degraded(reason),
        std::io::Cursor::new(input.into_bytes()),
        &mut output,
    )
    .await
    .expect("loop runs to EOF");
    String::from_utf8_lossy(&output)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect()
}

/// A runner that gets a dead stdio server drops the entry entirely, and then nothing can
/// tell the user why. So the handshake is answered locally even with no credential.
#[tokio::test]
async fn initialize_is_answered_locally_when_unauthenticated() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#],
    )
    .await;
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "opman-proxy:linear");
}

/// An empty tool list would mean the model never learns the server exists.
#[tokio::test]
async fn tools_list_offers_one_synthetic_tool_that_explains_itself() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#],
    )
    .await;
    let tools = out[0]["result"]["tools"].as_array().expect("array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "linear__authenticate");
    assert!(tools[0]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("opman mcp login linear"));
}

/// A successful result with `isError`, so the model relays it rather than the runner
/// surfacing an opaque transport failure.
#[tokio::test]
async fn a_tool_call_returns_an_actionable_result_not_a_protocol_error() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x"}}"#],
    )
    .await;
    assert!(out[0].get("error").is_none());
    assert_eq!(out[0]["result"]["isError"], true);
    assert!(out[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("opman mcp login linear"));
}

#[tokio::test]
async fn an_unconfigured_server_says_so_rather_than_asking_for_a_login() {
    let out = drive(
        DegradedReason::NotConfigured,
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"x"}}"#],
    )
    .await;
    let text = out[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(text.contains("not configured"));
}

#[tokio::test]
async fn a_notification_produces_no_response_even_when_degraded() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#,
        ],
    )
    .await;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["id"], 2);
}

#[tokio::test]
async fn resource_and_prompt_listings_are_valid_and_empty() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list"}"#,
        ],
    )
    .await;
    assert_eq!(out[0]["result"]["resources"], json!([]));
    assert_eq!(out[1]["result"]["prompts"], json!([]));
}

#[tokio::test]
async fn malformed_input_is_answered_and_the_loop_survives() {
    let out = drive(
        DegradedReason::NotAuthenticated,
        &[
            "{ not json",
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#,
        ],
    )
    .await;
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["error"]["code"], -32700);
}

/// An authenticated-mode proxy with no credential yet must still answer the handshake
/// locally, or the runner drops the server and the user never learns why.
#[test]
fn only_a_tool_call_is_worth_holding_open() {
    assert!(is_tool_call(&json!({ "method": "tools/call" })));
    assert!(!is_tool_call(&json!({ "method": "initialize" })));
    assert!(!is_tool_call(&json!({ "method": "tools/list" })));
}
