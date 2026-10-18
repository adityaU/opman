//! An in-process MCP server for tests, spoken to over in-memory pipes.
//!
//! Tools: `echo` (returns its arguments as JSON text), `add`, `sleep` (`ms`), `fail`
//! (an `isError` result), `image`, and `structured`. Every message it receives is
//! recorded, so tests can assert on the handshake itself.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex as AsyncMutex;

use super::client::{Connection, Entry, Opener, Pool};
use super::names::Namespace;

pub type Seen = Arc<Mutex<Vec<Value>>>;

/// The tool list the fake advertises.
pub fn tool_defs() -> Value {
    json!([
        { "name": "echo", "description": "Return the input.",
          "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } } } },
        { "name": "add", "description": "Add two numbers.",
          "inputSchema": { "type": "object",
            "properties": { "a": { "type": "number" }, "b": { "type": "number" } },
            "required": ["a", "b"] } },
        { "name": "sleep", "description": "Wait, then answer.",
          "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer" } } } },
        { "name": "fail", "description": "Always fails.", "inputSchema": { "type": "object" } },
        { "name": "image", "description": "Returns a picture.", "inputSchema": { "type": "object" } },
        { "name": "structured", "description": "Structured result.",
          "inputSchema": { "type": "object" },
          "outputSchema": { "type": "object", "properties": { "n": { "type": "number" } } } }
    ])
}

fn text(body: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": body }] })
}

async fn answer(name: &str, args: &Value) -> Value {
    match name {
        "echo" => text(&args.to_string()),
        "add" => {
            let sum = args["a"].as_f64().unwrap_or(0.0) + args["b"].as_f64().unwrap_or(0.0);
            text(&sum.to_string())
        }
        "sleep" => {
            tokio::time::sleep(Duration::from_millis(args["ms"].as_u64().unwrap_or(0))).await;
            text("slept")
        }
        "fail" => json!({ "content": [{ "type": "text", "text": "boom" }], "isError": true }),
        "image" => json!({ "content": [
            { "type": "text", "text": "here" },
            { "type": "image", "data": "AAAA", "mimeType": "image/png" }
        ] }),
        "structured" => json!({ "content": [], "structuredContent": { "n": 7 } }),
        _ => json!({ "content": [{ "type": "text", "text": "no such tool" }], "isError": true }),
    }
}

/// Start the fake; returns the client's ends and the record of what the server saw.
pub fn spawn_fake() -> (tokio::io::DuplexStream, tokio::io::DuplexStream, Seen) {
    let (client_out, server_in) = tokio::io::duplex(1 << 20);
    let (server_out, client_in) = tokio::io::duplex(1 << 20);
    let seen: Seen = Arc::default();
    let record = seen.clone();
    tokio::spawn(async move {
        let writer = Arc::new(AsyncMutex::new(server_out));
        let mut lines = BufReader::new(server_in).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            record.lock().map(|mut all| all.push(message.clone())).ok();
            let writer = writer.clone();
            tokio::spawn(async move { reply(message, writer).await });
        }
    });
    (client_in, client_out, seen)
}

async fn reply(message: Value, writer: Arc<AsyncMutex<tokio::io::DuplexStream>>) {
    let Some(id) = message.get("id").cloned() else {
        return;
    };
    let params = &message["params"];
    let result = match message["method"].as_str() {
        Some("initialize") => json!({
            "protocolVersion": "2025-06-18", "capabilities": { "tools": {} },
            "serverInfo": { "name": "fake", "version": "1" }
        }),
        Some("tools/list") => json!({ "tools": tool_defs() }),
        Some("tools/call") => {
            let name = params["name"].as_str().unwrap_or_default();
            answer(name, &params["arguments"]).await
        }
        _ => {
            let error = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "nope" } });
            send(&writer, &error).await;
            return;
        }
    };
    send(
        &writer,
        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    )
    .await;
}

pub async fn send(writer: &AsyncMutex<tokio::io::DuplexStream>, message: &Value) {
    let mut bytes = message.to_string().into_bytes();
    bytes.push(b'\n');
    let mut writer = writer.lock().await;
    let _ = writer.write_all(&bytes).await;
}

pub async fn connect() -> (Connection, Seen) {
    let (reader, writer, seen) = spawn_fake();
    let conn = Connection::open(reader, writer, None)
        .await
        .expect("fake handshake");
    (conn, seen)
}

/// A pool with the fake mounted as `name`; each open starts a fresh fake.
pub fn pool_with(name: &str) -> Pool {
    let open: Opener = Box::new(|| {
        Box::pin(async {
            let (reader, writer, _) = spawn_fake();
            Connection::open(reader, writer, None)
                .await
                .map_err(|e| e.to_string())
        })
    });
    let namespace = Namespace::for_server(name, &Default::default());
    Pool::new(vec![Entry::new(namespace, name, open)])
}

use futures::future::BoxFuture;

use super::sandbox::{Attachment, ToolHost, ToolOutput};

/// A host with no MCP behind it: namespace `m` with `echo`, `slow`, `fail`, `image`, `big`.
pub struct FakeHost {
    pub delay: Duration,
}

impl FakeHost {
    pub fn new(delay_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            delay: Duration::from_millis(delay_ms),
        })
    }
}

impl ToolHost for FakeHost {
    fn namespaces(&self) -> Vec<(String, Vec<String>)> {
        let names = ["echo", "slow", "fail", "image", "big"];
        vec![("m".into(), names.iter().map(|n| (*n).to_string()).collect())]
    }

    fn call<'a>(
        &'a self,
        _ns: &'a str,
        tool: &'a str,
        input: Value,
    ) -> BoxFuture<'a, Result<ToolOutput, String>> {
        Box::pin(async move {
            match tool {
                "echo" => Ok(ToolOutput::plain(input)),
                "slow" => {
                    tokio::time::sleep(self.delay).await;
                    Ok(ToolOutput::plain(json!("done")))
                }
                "fail" => Err("boom".into()),
                "image" => Ok(ToolOutput {
                    value: json!("pic"),
                    attachments: vec![Attachment {
                        mime: "image/png".into(),
                        data: "AAAA".into(),
                    }],
                }),
                "big" => Ok(ToolOutput::plain(json!("x".repeat(20_000)))),
                other => Err(format!("no tool {other}")),
            }
        })
    }

    fn search(&self, query: &str) -> Value {
        json!([{ "path": "tools.m.echo", "query": query }])
    }

    fn unknown(&self, namespace: &str, tool: Option<&str>) -> String {
        match tool {
            Some(tool) => format!("`{namespace}` has no tool named `{tool}`."),
            None => format!("There is no `{namespace}` namespace."),
        }
    }
}
