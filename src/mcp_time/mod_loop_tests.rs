//! Wave-2 coverage for the stdio read-loop and its single-line handler.
use super::*;
use serde_json::json;

#[test]
fn handle_line_empty_is_none() {
    assert!(handle_line("").is_none());
    assert!(handle_line("   \n").is_none());
}

#[test]
fn handle_line_parse_error() {
    let resp = handle_line("{ not json").unwrap();
    assert_eq!(resp["error"]["code"], -32700);
    assert!(resp["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Parse error"));
    assert_eq!(resp["id"], json!(null));
}

#[test]
fn handle_line_notification_is_none() {
    let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized","id":null}"#;
    assert!(handle_line(line).is_none());
}

#[test]
fn handle_line_routes_initialize() {
    let line = r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#;
    let resp = handle_line(line).unwrap();
    assert_eq!(resp["result"]["serverInfo"]["name"], "opman-time");
}

#[tokio::test]
async fn run_time_bridge_drives_full_loop() {
    // Covers: empty-line skip, parse error, initialize, notification (no reply),
    // tools/list, tools/call, unknown method, then EOF break.
    let input = concat!(
        "\n",
        "{ bad json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":1}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"id\":null}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":2}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"time_now\",\"arguments\":{}},\"id\":3}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"nope\",\"id\":4}\n",
    );
    let mut out: Vec<u8> = Vec::new();
    run_time_bridge(input.as_bytes(), &mut out).await.unwrap();
    let s = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    // 5 responses: parse error, initialize, tools/list, tools/call, unknown.
    assert_eq!(lines.len(), 5, "got: {s}");
    assert!(s.contains("Parse error"));
    assert!(s.contains("opman-time"));
    assert!(s.contains("Current time"));
    assert!(s.contains("Method not found: nope"));
}

#[tokio::test]
async fn run_time_bridge_immediate_eof() {
    let mut out: Vec<u8> = Vec::new();
    run_time_bridge(&b""[..], &mut out).await.unwrap();
    assert!(out.is_empty());
}

#[tokio::test]
async fn write_response_generic_over_vec() {
    let mut buf: Vec<u8> = Vec::new();
    write_response(&mut buf, &json!({"hello":"world"})).await;
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("\"hello\":\"world\""));
    assert!(s.ends_with('\n'));
}
