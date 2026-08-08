//! The probe against real child processes. `/bin/sh` stands in for a server: it prints
//! canned JSON-RPC frames and holds stdin open, which is every property the exchange
//! actually depends on.

use std::borrow::Cow;

use super::*;

/// A fake server that prints `script` and then parks on stdin.
///
/// Parking matters: a server that exits the moment it has written closes the pipe the
/// probe is still writing `notifications/initialized` into, and the resulting EPIPE would
/// be a property of the fake rather than of the code under test.
fn fake(script: &str) -> WireStdio<'static> {
    let program = format!("{script}\ncat >/dev/null");
    WireStdio {
        command: "/bin/sh",
        args: vec![Cow::Borrowed("-c"), Cow::Owned(program)],
        env: Vec::new(),
        cwd: None,
    }
}

fn line(value: serde_json::Value) -> String {
    format!("printf '%s\\n' {}", shell_quote(&value.to_string()))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn hello(id: u8) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "fake", "version": "9.9" },
        }
    })
}

#[tokio::test]
async fn a_listing_carries_the_server_identity_and_every_schema() {
    let listing = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": { "tools": [{
            "name": "echo",
            "description": "Say it back",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        }]}
    });
    let launch = fake(&format!("{}\n{}", line(hello(1)), line(listing)));

    let listed = list_tools(&launch).await.expect("probe should succeed");

    let server = listed.server.expect("serverInfo should be reported");
    assert_eq!(server.name, "fake");
    assert_eq!(server.version.as_deref(), Some("9.9"));
    assert_eq!(listed.tools.len(), 1);
    let tool = &listed.tools[0];
    assert_eq!(tool.name, "echo");
    assert_eq!(tool.description.as_deref(), Some("Say it back"));
    // The schema is the point of the feature — it must arrive whole, not summarised.
    assert_eq!(tool.input_schema["required"][0], "text");
    assert_eq!(tool.input_schema["properties"]["text"]["type"], "string");
}

#[tokio::test]
async fn frames_the_probe_did_not_ask_for_are_skipped() {
    let noise = json!({ "jsonrpc": "2.0", "method": "notifications/message", "params": {} });
    let stray = json!({ "jsonrpc": "2.0", "id": 77, "result": {} });
    let listing = json!({ "jsonrpc": "2.0", "id": 2, "result": { "tools": [] } });
    let launch = fake(&format!(
        "{}\n{}\n{}\n{}\n{}",
        line(noise.clone()),
        line(hello(1)),
        line(stray),
        line(noise),
        line(listing)
    ));

    let listed = list_tools(&launch).await.expect("probe should succeed");

    assert!(listed.tools.is_empty());
}

#[tokio::test]
async fn a_refused_handshake_names_the_method_and_the_reason() {
    let refusal = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": -32002, "message": "needs login" }
    });
    let launch = fake(&line(refusal));

    let error = list_tools(&launch)
        .await
        .expect_err("a refusal is not a listing");

    let text = format!("{error:#}");
    assert!(text.contains("initialize"), "{text}");
    assert!(text.contains("needs login"), "{text}");
}

#[tokio::test]
async fn a_server_that_exits_early_is_reported_as_such() {
    // No `cat`: this one really does close its pipes.
    let launch = WireStdio {
        command: "/bin/sh",
        args: vec![Cow::Borrowed("-c"), Cow::Borrowed("exit 0")],
        env: Vec::new(),
        cwd: None,
    };

    let error = list_tools(&launch)
        .await
        .expect_err("an immediate exit is not a listing");

    assert!(format!("{error:#}").contains("exited"), "{error:#}");
}

#[tokio::test]
async fn a_command_that_does_not_exist_fails_at_spawn() {
    let launch = WireStdio {
        command: "/nonexistent/opman-probe-target",
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
    };

    let error = list_tools(&launch)
        .await
        .expect_err("a missing binary is not a listing");

    assert!(
        format!("{error:#}").contains("failed to launch"),
        "{error:#}"
    );
}

#[test]
fn one_unreadable_entry_does_not_take_the_listing_with_it() {
    let listed = json!({ "tools": [
        { "not-a": "tool" },
        { "name": "good", "inputSchema": { "type": "object" } },
    ]});

    let parsed = tools(listed).expect("a bad entry is not a bad listing");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, "good");
}

#[test]
fn a_listing_without_a_tools_array_is_an_error_not_an_empty_list() {
    assert!(tools(json!({ "resources": [] })).is_err());
}
