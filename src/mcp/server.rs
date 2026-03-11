use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::nvim_handler::handle_nvim_op_blocking;
use super::types::{
    NvimSocketRegistry, PendingSocketRequest, SocketRequest, SocketResponse,
};

/// Update MCP activity timestamp.
fn update_activity(ts: &AtomicU64) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    ts.store(now, Ordering::Release);
}

/// Spawn the Unix domain socket server for a single project.
/// Handles concurrency controls (ephemeral dedup, per-file nvim locks,
/// per-tab terminal locks) and direct nvim dispatch when possible.
pub fn spawn_socket_server(
    project_path: &Path,
    request_tx: mpsc::UnboundedSender<crate::app::BackgroundEvent>,
    project_idx: usize,
    nvim_registry: NvimSocketRegistry,
    last_mcp_activity_ms: Arc<AtomicU64>,
) -> PathBuf {
    let sock_path = super::types::socket_path_for_project(project_path);

    // Remove stale socket file if it exists
    let _ = std::fs::remove_file(&sock_path);

    let sock = sock_path.clone();
    tokio::spawn(async move {
        let listener = match UnixListener::bind(&sock) {
            Ok(l) => {
                info!(?sock, "MCP socket server listening");
                l
            }
            Err(e) => {
                warn!(?sock, "Failed to bind MCP socket: {}", e);
                return;
            }
        };

        // Tracks which ephemeral task names currently have an in-flight run.
        let busy_ephemeral: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        // Per-file lock map for neovim operations.
        let nvim_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Per-tab lock map for terminal operations.
        let term_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("MCP socket accept error: {}", e);
                    continue;
                }
            };

            let tx = request_tx.clone();
            let pidx = project_idx;
            let eph = busy_ephemeral.clone();
            let nvim = nvim_locks.clone();
            let term = term_locks.clone();
            let registry = nvim_registry.clone();
            let activity_ms = last_mcp_activity_ms.clone();

            tokio::spawn(async move {
                handle_connection(stream, tx, pidx, eph, nvim, term, registry, activity_ms).await;
            });
        }
    });

    sock_path
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    tx: mpsc::UnboundedSender<crate::app::BackgroundEvent>,
    pidx: usize,
    eph: Arc<Mutex<HashSet<String>>>,
    nvim: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    term: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    registry: NvimSocketRegistry,
    activity_ms: Arc<AtomicU64>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // Read one JSON line per connection
    match buf_reader.read_line(&mut line).await {
        Ok(0) => return, // EOF
        Ok(_) => {}
        Err(e) => {
            debug!("MCP socket read error: {}", e);
            return;
        }
    }

    let request: SocketRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let resp = SocketResponse::err(format!("Invalid JSON: {}", e));
            let _ = writer
                .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                .await;
            let _ = writer.write_all(b"\n").await;
            return;
        }
    };

    // Mark MCP activity on request arrival.
    update_activity(&activity_ms);

    // Handle ephemeral_lock / ephemeral_unlock directly (no main-loop round-trip)
    match request.op.as_str() {
        "ephemeral_lock" => {
            let name = match &request.name {
                Some(n) => n.clone(),
                None => {
                    let resp = SocketResponse::err("Missing 'name' for ephemeral_lock".into());
                    let _ = writer
                        .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                        .await;
                    let _ = writer.write_all(b"\n").await;
                    return;
                }
            };
            let acquired = { eph.lock().unwrap().insert(name.clone()) };
            let resp = if acquired {
                SocketResponse::ok_empty()
            } else {
                SocketResponse::err(format!(
                    "Ephemeral task \"{}\" is already running. \
                     Use a different name to run in parallel, or wait for it to complete.",
                    name
                ))
            };
            let _ = writer
                .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                .await;
            let _ = writer.write_all(b"\n").await;
            return;
        }
        "ephemeral_unlock" => {
            if let Some(name) = &request.name {
                eph.lock().unwrap().remove(name);
            }
            let resp = SocketResponse::ok_empty();
            let _ = writer
                .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                .await;
            let _ = writer.write_all(b"\n").await;
            return;
        }
        _ => {}
    }

    // Acquire per-file neovim lock(s) for nvim_* ops.
    let is_nvim_op = request.op.starts_with("nvim_");
    let nvim_lock_keys: Vec<String> = if is_nvim_op {
        if let Some(ref edits) = request.edits {
            let mut paths: Vec<String> = edits
                .iter()
                .map(|e| e.file_path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if paths.is_empty() {
                paths.push("__current__".to_string());
            }
            paths
        } else {
            vec![request
                .file_path
                .as_deref()
                .unwrap_or("__current__")
                .to_string()]
        }
    } else {
        vec![]
    };
    let nvim_arcs: Vec<Arc<tokio::sync::Mutex<()>>> = {
        if nvim_lock_keys.is_empty() {
            vec![]
        } else {
            let mut locks_map = nvim.lock().unwrap();
            nvim_lock_keys
                .iter()
                .map(|key| {
                    locks_map
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                        .clone()
                })
                .collect()
        }
    };
    let mut _nvim_guards = Vec::new();
    for arc in &nvim_arcs {
        _nvim_guards.push(arc.lock().await);
    }

    // Acquire per-tab terminal lock for terminal ops.
    let is_term_op = matches!(request.op.as_str(), "run" | "read" | "close" | "rename");
    let _term_arc = if is_term_op {
        let key = request
            .tab
            .map(|t| t.to_string())
            .unwrap_or_else(|| "__active__".to_string());
        let lock = {
            let mut locks = term.lock().unwrap();
            locks
                .entry(key)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        Some(lock)
    } else {
        None
    };
    let _term_guard = match &_term_arc {
        Some(lock) => Some(lock.lock().await),
        None => None,
    };

    // For nvim_* ops: try to handle directly using the registry.
    if is_nvim_op {
        let session_id = request.session_id.clone().unwrap_or_default();
        let nvim_socket = {
            let reg = registry.read().await;
            reg.get(&(pidx, session_id.clone())).cloned()
        };

        if let Some(nvim_socket) = nvim_socket {
            let response = {
                let req = request;
                let sock = nvim_socket;
                tokio::task::spawn_blocking(move || handle_nvim_op_blocking(&sock, &req))
                    .await
                    .unwrap_or_else(|e| SocketResponse::err(format!("Task join error: {}", e)))
            };

            let json = serde_json::to_string(&response).unwrap();
            let _ = writer.write_all(json.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            update_activity(&activity_ms);

            drop(_nvim_guards);
            return;
        }
    }

    // Send to main event loop and wait for response
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let session_id = request.session_id.clone().unwrap_or_default();
    let pending = PendingSocketRequest { request, reply_tx };
    let _ = tx.send(crate::app::BackgroundEvent::McpSocketRequest {
        project_idx: pidx,
        session_id,
        pending,
    });

    let write_result = match reply_rx.await {
        Ok(response) => {
            let json = serde_json::to_string(&response).unwrap();
            let r1 = writer.write_all(json.as_bytes()).await;
            let r2 = writer.write_all(b"\n").await;
            r1.and(r2)
        }
        Err(_) => {
            let resp = SocketResponse::err("Internal error: no response".into());
            let _ = writer
                .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                .await;
            let _ = writer.write_all(b"\n").await;
            Ok(())
        }
    };
    let _ = write_result;
    update_activity(&activity_ms);

    drop(_nvim_guards);
    drop(_term_guard);
}
