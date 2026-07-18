//! Wave-2 coverage for the UI-bridge stdio read-loop and single-line handler.
use super::*;
use serde_json::json;

#[test]
fn handle_line_blank_none() {
    assert!(handle_line("\n").is_none());
    assert!(handle_line("").is_none());
}

#[test]
fn handle_line_parse_error() {
    let resp = handle_line("}}}bad").unwrap();
    assert_eq!(resp["error"]["code"], -32700);
    assert_eq!(resp["id"], json!(null));
}

#[test]
fn handle_line_notification_none() {
    let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized","id":0}"#;
    assert!(handle_line(line).is_none());
}

#[tokio::test]
async fn run_ui_bridge_drives_full_loop() {
    let input = concat!(
        "   \n",
        "not-json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\",\"id\":1}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"id\":0}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":2}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"ui_render\",\"arguments\":{\"title\":\"T\",\"blocks\":[{\"type\":\"card\",\"data\":{}}]}},\"id\":3}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"weird\",\"id\":4}\n",
    );
    let mut out: Vec<u8> = Vec::new();
    run_ui_bridge(input.as_bytes(), &mut out).await.unwrap();
    let s = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 5, "got: {s}");
    assert!(s.contains("Parse error"));
    assert!(s.contains("opman-ui"));
    assert!(s.contains("Rendered UI"));
    assert!(s.contains("Method not found: weird"));
}

#[tokio::test]
async fn run_ui_bridge_eof() {
    let mut out: Vec<u8> = Vec::new();
    run_ui_bridge(&b""[..], &mut out).await.unwrap();
    assert!(out.is_empty());
}

#[tokio::test]
async fn write_response_generic_vec() {
    let mut buf: Vec<u8> = Vec::new();
    write_response(&mut buf, &json!({"k":1})).await;
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("\"k\":1"));
    assert!(s.ends_with('\n'));
}
