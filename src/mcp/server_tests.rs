use super::*;
use crate::app::BackgroundEvent;
use crate::mcp::socket_client::send_socket_request;
use crate::mcp::types::{new_nvim_socket_registry, SocketRequest, SocketResponse};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_project() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("opman-srv-proj-{}-{}", std::process::id(), n))
}

struct Harness {
    sock: PathBuf,
    rx: mpsc::UnboundedReceiver<BackgroundEvent>,
    activity: Arc<AtomicU64>,
    project: PathBuf,
    registry: crate::mcp::NvimSocketRegistry,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.sock);
    }
}

fn start() -> Harness {
    let project = unique_project();
    let (tx, rx) = mpsc::unbounded_channel();
    let activity = Arc::new(AtomicU64::new(0));
    let registry = new_nvim_socket_registry();
    let sock = spawn_socket_server(&project, tx, 0, registry.clone(), activity.clone());
    Harness {
        sock,
        rx,
        activity,
        project,
        registry,
    }
}

/// Wait until the socket is accepting connections (also exercises the EOF path,
/// since the probe connection closes without sending a request line).
async fn wait_ready(sock: &Path) {
    for _ in 0..400 {
        if tokio::net::UnixStream::connect(sock).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("socket never became ready");
}

async fn raw_send(sock: &Path, payload: &str) -> String {
    let mut stream = tokio::net::UnixStream::connect(sock).await.unwrap();
    stream.write_all(payload.as_bytes()).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
    stream.flush().await.unwrap();
    let (r, _w) = stream.into_split();
    let mut br = BufReader::new(r);
    let mut buf = String::new();
    br.read_line(&mut buf).await.unwrap();
    buf
}

#[tokio::test]
async fn update_activity_sets_timestamp() {
    let ts = AtomicU64::new(0);
    update_activity(&ts);
    assert!(ts.load(Ordering::Acquire) > 0);
}

#[tokio::test]
async fn invalid_json_returns_error() {
    let h = start();
    wait_ready(&h.sock).await;
    let resp = raw_send(&h.sock, "{not valid json").await;
    assert!(resp.contains("Invalid JSON"));
}

#[tokio::test]
async fn ephemeral_lock_and_duplicate() {
    let h = start();
    wait_ready(&h.sock).await;
    let lock = SocketRequest {
        op: "ephemeral_lock".into(),
        name: Some("build".into()),
        ..Default::default()
    };
    let first = send_socket_request(&h.sock, &lock).await.unwrap();
    assert!(first.ok);
    let second = send_socket_request(&h.sock, &lock).await.unwrap();
    assert!(!second.ok);
    assert!(second.error.unwrap().contains("already running"));
    // activity timestamp should have been recorded
    assert!(h.activity.load(Ordering::Acquire) > 0);
}

#[tokio::test]
async fn ephemeral_lock_missing_name() {
    let h = start();
    wait_ready(&h.sock).await;
    let req = SocketRequest {
        op: "ephemeral_lock".into(),
        ..Default::default()
    };
    let resp = send_socket_request(&h.sock, &req).await.unwrap();
    assert!(!resp.ok);
    assert!(resp.error.unwrap().contains("Missing 'name'"));
}

#[tokio::test]
async fn ephemeral_unlock_with_and_without_name() {
    let h = start();
    wait_ready(&h.sock).await;
    let with = SocketRequest {
        op: "ephemeral_unlock".into(),
        name: Some("x".into()),
        ..Default::default()
    };
    assert!(send_socket_request(&h.sock, &with).await.unwrap().ok);
    let without = SocketRequest {
        op: "ephemeral_unlock".into(),
        ..Default::default()
    };
    assert!(send_socket_request(&h.sock, &without).await.unwrap().ok);
}

#[tokio::test]
async fn terminal_op_routes_to_main_loop() {
    let mut h = start();
    wait_ready(&h.sock).await;
    let sock = h.sock.clone();
    let handle = tokio::spawn(async move {
        let req = SocketRequest {
            op: "read".into(),
            tab: Some(1),
            ..Default::default()
        };
        send_socket_request(&sock, &req).await
    });
    // Receive the forwarded request and reply.
    let ev = h.rx.recv().await.unwrap();
    match ev {
        BackgroundEvent::McpSocketRequest { pending, .. } => {
            pending
                .reply_tx
                .send(SocketResponse::ok_text("from main loop".into()))
                .unwrap();
        }
        _ => panic!("unexpected event"),
    }
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.output.as_deref(), Some("from main loop"));
}

#[tokio::test]
async fn main_loop_reply_dropped_gives_internal_error() {
    let mut h = start();
    wait_ready(&h.sock).await;
    let sock = h.sock.clone();
    let handle = tokio::spawn(async move {
        let req = SocketRequest {
            op: "close".into(),
            ..Default::default()
        };
        send_socket_request(&sock, &req).await
    });
    let ev = h.rx.recv().await.unwrap();
    // Drop the pending request (and its reply sender) without replying.
    drop(ev);
    let resp = handle.await.unwrap().unwrap();
    assert!(!resp.ok);
    assert!(resp.error.unwrap().contains("no response"));
}

#[tokio::test]
async fn nvim_op_without_registry_routes_to_main_loop() {
    let mut h = start();
    wait_ready(&h.sock).await;
    let sock = h.sock.clone();
    let handle = tokio::spawn(async move {
        let req = SocketRequest {
            op: "nvim_info".into(),
            ..Default::default()
        };
        send_socket_request(&sock, &req).await
    });
    let ev = h.rx.recv().await.unwrap();
    match ev {
        BackgroundEvent::McpSocketRequest { pending, .. } => {
            pending
                .reply_tx
                .send(SocketResponse::ok_text("nvim via main".into()))
                .unwrap();
        }
        _ => panic!("unexpected event"),
    }
    let resp = handle.await.unwrap().unwrap();
    assert_eq!(resp.output.as_deref(), Some("nvim via main"));
}

#[tokio::test]
async fn nvim_op_with_registry_dispatches_directly() {
    let h = start();
    wait_ready(&h.sock).await;
    // Register a (bogus) neovim socket for (project_idx=0, session_id="").
    let bad_nvim = std::env::temp_dir().join("opman-srv-nvim-missing.sock");
    let _ = std::fs::remove_file(&bad_nvim);
    h.registry
        .write()
        .await
        .insert((0, String::new()), bad_nvim);

    // nvim_info tolerates RPC failures (uses defaults) → returns ok directly,
    // without any main-loop round-trip.
    let req = SocketRequest {
        op: "nvim_info".into(),
        ..Default::default()
    };
    let resp = send_socket_request(&h.sock, &req).await.unwrap();
    assert!(resp.ok);
    assert!(resp.output.unwrap().contains("Buffer:"));
    // No event should have been forwarded to the main loop; keep project alive.
    let _ = &h.project;
}
