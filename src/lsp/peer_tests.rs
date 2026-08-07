//! Peer tests drive a scripted server over `tokio::io::duplex`, so there is no
//! process, no timing luck, and no installed binary required.

use super::*;
use std::sync::Mutex as StdMutex;
use tokio::io::BufReader;

use crate::lsp::framing::{read_frame, write_frame};

/// Records what the handler was asked, and answers requests from a script.
#[derive(Default)]
struct Recorder {
    notifications: StdMutex<Vec<(String, Value)>>,
    reply: Option<Value>,
}

impl Handler for Recorder {
    fn request(&self, _method: &str, _params: &Value) -> Result<Value> {
        Ok(self.reply.clone().unwrap_or(Value::Null))
    }
    fn notify(&self, method: &str, params: Value) {
        if let Ok(mut seen) = self.notifications.lock() {
            seen.push((method.to_string(), params));
        }
    }
}

/// Wire a peer to a fake server end. Returns the peer, the server's reader and
/// writer, and the handler for assertions.
fn wire(handler: Arc<Recorder>) -> (Peer, tokio::io::DuplexStream) {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    (Peer::new(ours, handler), theirs)
}

#[tokio::test]
async fn request_resolves_with_the_matching_response() {
    let (peer, server) = wire(Arc::new(Recorder::default()));
    let (server_read, mut server_write) = tokio::io::split(server);

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let frame = read_frame(&mut reader).await.unwrap().unwrap();
        let id = frame["id"].clone();
        write_frame(
            &mut server_write,
            &json!({ "jsonrpc": "2.0", "id": id, "result": { "ok": true } }),
        )
        .await
        .unwrap();
    });

    let result = peer
        .request("initialize", json!({}), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(result["ok"], true);
}

/// Responses may arrive in any order; each must wake its own caller.
#[tokio::test]
async fn out_of_order_responses_reach_the_right_waiters() {
    let (peer, server) = wire(Arc::new(Recorder::default()));
    let (server_read, mut server_write) = tokio::io::split(server);

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let first = read_frame(&mut reader).await.unwrap().unwrap();
        let second = read_frame(&mut reader).await.unwrap().unwrap();
        // Answer the second request first.
        for frame in [second, first] {
            let id = frame["id"].clone();
            let method = frame["method"].as_str().unwrap().to_string();
            write_frame(
                &mut server_write,
                &json!({ "jsonrpc": "2.0", "id": id, "result": method }),
            )
            .await
            .unwrap();
        }
    });

    let a = peer.request("alpha", json!({}), Duration::from_secs(5));
    let b = peer.request("beta", json!({}), Duration::from_secs(5));
    let (a, b) = tokio::join!(a, b);
    assert_eq!(a.unwrap(), "alpha");
    assert_eq!(b.unwrap(), "beta");
}

/// An error response becomes an `Err`, carrying the server's message.
#[tokio::test]
async fn error_responses_surface_the_message() {
    let (peer, server) = wire(Arc::new(Recorder::default()));
    let (server_read, mut server_write) = tokio::io::split(server);

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let frame = read_frame(&mut reader).await.unwrap().unwrap();
        write_frame(
            &mut server_write,
            &json!({
                "jsonrpc": "2.0", "id": frame["id"],
                "error": { "code": -32601, "message": "method not found" }
            }),
        )
        .await
        .unwrap();
    });

    let err = peer
        .request("nope", json!({}), Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("method not found"));
}

/// The bug this prevents: a server dies and every in-flight request hangs
/// forever, pinning an axum task each.
#[tokio::test]
async fn pending_requests_fail_when_the_server_exits() {
    let (peer, server) = wire(Arc::new(Recorder::default()));
    let (server_read, server_write) = tokio::io::split(server);

    tokio::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let _ = read_frame(&mut reader).await;
        drop(server_write); // server exits without answering
    });

    let err = peer
        .request("hover", json!({}), Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("exited") || err.to_string().contains("closed"));
    assert!(!peer.is_alive());
}

/// A wedged server must not hold the caller past the timeout.
#[tokio::test]
async fn requests_give_up_after_the_timeout() {
    let (peer, server) = wire(Arc::new(Recorder::default()));
    // Hold the server end open but never answer.
    let _keep_alive = server;

    let err = peer
        .request("hover", json!({}), Duration::from_millis(120))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("timed out"));
}

/// Server→client notifications reach the handler.
#[tokio::test]
async fn notifications_reach_the_handler() {
    let handler = Arc::new(Recorder::default());
    let (peer, server) = wire(handler.clone());
    let (_server_read, mut server_write) = tokio::io::split(server);

    write_frame(
        &mut server_write,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": "file:///x.rs" }
        }),
    )
    .await
    .unwrap();

    for _ in 0..50 {
        if !handler.notifications.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let seen = handler.notifications.lock().unwrap();
    assert_eq!(seen[0].0, "textDocument/publishDiagnostics");
    assert!(peer.is_alive());
}

/// Server→client *requests* must be answered, or servers that ask for
/// configuration during startup block forever.
#[tokio::test]
async fn inbound_requests_are_answered() {
    let handler = Arc::new(Recorder {
        notifications: StdMutex::new(Vec::new()),
        reply: Some(json!([null])),
    });
    let (_peer, server) = wire(handler);
    let (server_read, mut server_write) = tokio::io::split(server);

    write_frame(
        &mut server_write,
        &json!({
            "jsonrpc": "2.0", "id": 7,
            "method": "workspace/configuration",
            "params": { "items": [{ "section": "rust-analyzer" }] }
        }),
    )
    .await
    .unwrap();

    let mut reader = BufReader::new(server_read);
    let reply = read_frame(&mut reader).await.unwrap().unwrap();
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"], json!([null]));
}
