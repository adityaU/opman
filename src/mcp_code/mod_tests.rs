//! The whole server over in-memory pipes, against the fake MCP server.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;
use crate::mcp_code::testing::pool_with;

struct Rig {
    to_server: tokio::io::DuplexStream,
    from_server: tokio::io::Lines<BufReader<tokio::io::DuplexStream>>,
}

fn start() -> Rig {
    let (to_server, server_in) = tokio::io::duplex(1 << 20);
    let (server_out, from_server) = tokio::io::duplex(1 << 20);
    let pool = Arc::new(pool_with("fake"));
    tokio::spawn(async move {
        let _ = serve(
            pool,
            server_in,
            Arc::new(tokio::sync::Mutex::new(server_out)),
        )
        .await;
    });
    Rig {
        to_server,
        from_server: BufReader::new(from_server).lines(),
    }
}

impl Rig {
    async fn send(&mut self, message: Value) {
        self.to_server
            .write_all(format!("{message}\n").as_bytes())
            .await
            .expect("write");
    }

    async fn next(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(10), self.from_server.next_line())
            .await
            .expect("reply in time")
            .expect("read")
            .expect("a line");
        serde_json::from_str(&line).expect("json")
    }

    async fn call(&mut self, id: u64, code: &str) -> Value {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": "execute", "arguments": {"code": code}}}))
            .await;
        self.next().await
    }
}

#[tokio::test]
async fn full_protocol_session() {
    let mut rig = start();
    rig.send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}))
        .await;
    let hello = rig.next().await;
    assert_eq!(hello["result"]["serverInfo"]["name"], "opman-code");
    assert!(hello["result"]["capabilities"]["tools"].is_object());

    // `initialized` is a notification: no reply, so the next line is the list.
    rig.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;
    rig.send(json!({"jsonrpc": "2.0", "method": "notifications/unknown"}))
        .await;
    rig.send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))
        .await;
    let list = rig.next().await;
    assert_eq!(list["id"], 2);
    let tools = list["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "execute");
    assert!(tools[0]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("tools.fake.add"));

    let reply = rig
        .call(3, "return await tools.fake.add({a: 4, b: 5})")
        .await;
    assert_eq!(reply["id"], 3);
    assert_eq!(reply["result"]["content"][0]["text"], "9");
}

#[tokio::test]
async fn bad_requests_get_errors() {
    let mut rig = start();
    rig.to_server.write_all(b"not json\n").await.expect("write");
    assert_eq!(rig.next().await["error"]["code"], -32700);
    rig.send(json!({"jsonrpc": "2.0", "id": 5, "method": "nope"}))
        .await;
    assert_eq!(rig.next().await["error"]["code"], -32601);

    let reply = rig.call(6, "return 1").await;
    assert_eq!(reply["result"]["content"][0]["text"], "1");
    rig.send(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "other", "arguments": {}}}))
        .await;
    let unknown = rig.next().await;
    assert_eq!(unknown["result"]["isError"], true);
    rig.send(json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call",
        "params": {"name": "execute", "arguments": {}}}))
        .await;
    assert_eq!(rig.next().await["result"]["isError"], true);
}

#[tokio::test]
async fn cancelling_a_running_script_answers_it() {
    let mut rig = start();
    rig.send(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "execute", "arguments": {"code": "await tools.fake.sleep({ms: 5000})"}}})).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    rig.send(
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 9}}),
    )
    .await;
    let reply = rig.next().await;
    assert_eq!(reply["id"], 9);
    assert_eq!(
        reply["result"]["content"][0]["text"],
        "Execution cancelled."
    );
}

#[tokio::test]
async fn calls_do_not_block_each_other() {
    let mut rig = start();
    rig.send(json!({"jsonrpc": "2.0", "id": 10, "method": "tools/call",
        "params": {"name": "execute", "arguments": {"code": "await tools.fake.sleep({ms: 600}); return 'slow'"}}})).await;
    rig.send(json!({"jsonrpc": "2.0", "id": 11, "method": "tools/call",
        "params": {"name": "execute", "arguments": {"code": "return 'fast'"}}}))
        .await;
    let first = rig.next().await;
    assert_eq!(first["id"], 11);
    let second = rig.next().await;
    assert_eq!(second["id"], 10);
}
