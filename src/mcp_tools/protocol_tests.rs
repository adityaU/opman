//! The stdio protocol end to end, in memory: handshake, listing, calls, cancel.

use super::serve;
use super::workspace::Workspace;
use serde_json::{json, Value};
use tokio::io::{duplex, AsyncBufReadExt, AsyncWriteExt, BufReader};

struct Client {
    to_server: tokio::io::DuplexStream,
    from_server: tokio::io::Lines<BufReader<tokio::io::DuplexStream>>,
    _dir: tempfile::TempDir,
}

fn start() -> Client {
    let dir = tempfile::tempdir().expect("test fixture or assertion");
    let (client_out, server_in) = duplex(64 * 1024);
    let (server_out, client_in) = duplex(64 * 1024);
    let ws = Workspace::new(dir.path().to_path_buf());
    tokio::spawn(serve(server_in, server_out, ws));
    Client {
        to_server: client_out,
        from_server: BufReader::new(client_in).lines(),
        _dir: dir,
    }
}

impl Client {
    async fn send(&mut self, frame: Value) {
        self.to_server
            .write_all(format!("{frame}\n").as_bytes())
            .await
            .expect("test fixture or assertion");
    }

    async fn next(&mut self) -> Value {
        let line = self
            .from_server
            .next_line()
            .await
            .expect("test fixture or assertion")
            .expect("server closed");
        serde_json::from_str(&line).expect("test fixture or assertion")
    }
}

#[tokio::test]
async fn handshake_list_and_call() {
    let mut c = start();
    c.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}})).await;
    let init = c.next().await;
    assert_eq!(init["result"]["serverInfo"]["name"], "opman-tools");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");

    // The initialized notification must get no reply: the next frame is the list.
    c.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await;
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    let list = c.next().await;
    assert_eq!(list["id"], 2);
    let tools = list["result"]["tools"]
        .as_array()
        .expect("test fixture or assertion");
    let names: Vec<_> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("test fixture or assertion"))
        .collect();
    assert_eq!(
        names,
        [
            "bash",
            "read_file",
            "write_file",
            "edit_file",
            "list_dir",
            "glob",
            "grep",
            "web_fetch",
            "wait",
            "scheduler_list",
            "scheduler_create",
            "scheduler_update",
            "scheduler_delete",
            "scheduler_run"
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{tool}");
        assert!(
            tool["description"]
                .as_str()
                .expect("test fixture or assertion")
                .len()
                > 40
        );
        let name = tool["name"].as_str().expect("test fixture or assertion");
        assert!(name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'));
    }

    c.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"write_file","arguments":{"path":"x.txt","content":"hi\n"}}})).await;
    let wrote = c.next().await;
    assert_eq!(wrote["result"]["isError"], false);
    assert_eq!(wrote["result"]["structuredContent"]["created"], true);
    c.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bash","arguments":{"command":"cat x.txt"}}})).await;
    let ran = c.next().await;
    assert_eq!(ran["result"]["structuredContent"]["stdout"], "hi\n");
}

#[tokio::test]
async fn errors_are_results_not_protocol_errors() {
    let mut c = start();
    c.send(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"missing"}}})).await;
    let missing = c.next().await;
    assert_eq!(missing["result"]["isError"], true);
    assert!(missing["result"].get("structuredContent").is_none());
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nope","arguments":{}}})).await;
    assert!(c.next().await["result"]["content"][0]["text"]
        .as_str()
        .expect("test fixture or assertion")
        .contains("Unknown tool"));
    c.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read_file","arguments":{"path":5}}})).await;
    assert!(c.next().await["result"]["content"][0]["text"]
        .as_str()
        .expect("test fixture or assertion")
        .contains("invalid arguments"));
    c.send(json!({"jsonrpc":"2.0","id":4,"method":"what"}))
        .await;
    assert_eq!(c.next().await["error"]["code"], -32601);
    c.to_server
        .write_all(b"{ nope\n")
        .await
        .expect("test fixture or assertion");
    assert_eq!(c.next().await["error"]["code"], -32700);
    c.send(json!({"jsonrpc":"2.0","id":5,"method":"ping"}))
        .await;
    assert_eq!(c.next().await["result"], json!({}));
}

#[tokio::test]
async fn a_slow_call_does_not_block_others_and_cancel_stops_it() {
    let mut c = start();
    let marker = c._dir.path().join("alive");
    let script = format!("sleep 1; touch {}", marker.display());
    c.send(json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"bash","arguments":{"command":script}}})).await;
    c.send(json!({"jsonrpc":"2.0","id":11,"method":"tools/list"}))
        .await;
    assert_eq!(c.next().await["id"], 11);
    c.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":10}}))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
    assert!(!marker.exists(), "cancelled command still ran");
    c.send(json!({"jsonrpc":"2.0","id":12,"method":"ping"}))
        .await;
    assert_eq!(c.next().await["id"], 12);
}

#[tokio::test]
async fn wait_is_concurrent_and_accepts_cancellation() {
    let mut c = start();
    c.send(json!({"jsonrpc":"2.0","id":20,"method":"tools/call",
        "params":{"name":"wait","arguments":{"seconds":600}}}))
        .await;
    c.send(json!({"jsonrpc":"2.0","id":21,"method":"ping"}))
        .await;
    let ping = tokio::time::timeout(std::time::Duration::from_secs(3), c.next())
        .await
        .expect("wait must not block the request loop");
    assert_eq!(ping["id"], 21);
    c.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled",
        "params":{"requestId":20}}))
        .await;
    c.send(json!({"jsonrpc":"2.0","id":22,"method":"tools/call",
        "params":{"name":"wait","arguments":{"seconds":0}}}))
        .await;
    let completed = tokio::time::timeout(std::time::Duration::from_secs(3), c.next())
        .await
        .expect("a fresh wait should complete after cancellation");
    assert_eq!(completed["id"], 22);
    assert_eq!(completed["result"]["isError"], false);
    assert!(completed["result"]["structuredContent"]["waited_seconds"].is_number());
}
