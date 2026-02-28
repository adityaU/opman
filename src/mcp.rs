use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// ─── Shared neovim socket registry ──────────────────────────────────────────

/// Shared registry of neovim socket paths, keyed by (project_idx, session_id).
/// Updated by the main loop when neovim PTYs are spawned; read by socket server
/// tasks to handle nvim operations directly without round-tripping through the
/// main event loop.
pub type NvimSocketRegistry = Arc<tokio::sync::RwLock<HashMap<(usize, String), PathBuf>>>;

/// Create a new empty neovim socket registry.
pub fn new_nvim_socket_registry() -> NvimSocketRegistry {
    Arc::new(tokio::sync::RwLock::new(HashMap::new()))
}

// ─── Internal socket protocol ───────────────────────────────────────────────

/// A single edit operation within a multi-edit batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOp {
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub new_text: String,
}

/// Request sent over Unix socket from MCP bridge → manager.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SocketRequest {
    pub op: String, // "read" | "run" | "list" | "new" | "close" | "rename"
    // + neovim ops: "nvim_open" | "nvim_read" | "nvim_command" | "nvim_buffers" | "nvim_info"
    //   "nvim_diagnostics" | "nvim_definition" | "nvim_references"
    //   "nvim_hover" | "nvim_symbols" | "nvim_code_actions"
    //   "nvim_eval" | "nvim_grep" | "nvim_diff" | "nvim_write"
    //   "nvim_edit_and_save" | "nvim_undo" | "nvim_rename" | "nvim_format" | "nvim_signature"
    /// Session ID for routing to the correct per-session resources.
    /// Set by MCP bridges from OPENCODE_SESSION_ID env var.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tab: Option<usize>, // tab index (0-based)
    #[serde(default)]
    pub command: Option<String>, // for "run" op and "nvim_command" / "nvim_eval"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>, // for "new" and "rename" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<bool>, // for "run" op: wait for output to settle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_n: Option<usize>, // for "read" op: return only last N lines
    // ── Neovim-specific fields ──────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>, // for "nvim_open" / "nvim_grep" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>, // for "nvim_open" / "nvim_read" / LSP position ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>, // for "nvim_read" op (end of range)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<i64>, // column for LSP position ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>, // for "nvim_symbols" / "nvim_grep" ops
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buf_only: Option<bool>, // for "nvim_diagnostics": current buffer only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<bool>, // for "nvim_symbols": workspace vs document
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<bool>, // for "nvim_write": write all buffers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glob: Option<String>, // for "nvim_grep": file glob pattern
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_text: Option<String>, // for "nvim_edit_and_save": replacement text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>, // for "nvim_undo": undo count (negative = redo)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>, // for "nvim_rename": new symbol name
    // ── Multi-edit batch ────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<EditOp>>, // for "nvim_edit_and_save": batch of edits
}

/// Response sent over Unix socket from manager → MCP bridge.
#[derive(Debug, Serialize, Deserialize)]
pub struct SocketResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabInfo {
    pub index: usize,
    pub active: bool,
    pub name: String,
}

impl SocketResponse {
    pub fn ok_text(output: String) -> Self {
        Self {
            ok: true,
            output: Some(output),
            tabs: None,
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_tabs(tabs: Vec<TabInfo>) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: Some(tabs),
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_tab_created(tab_index: usize) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: Some(tab_index),
            command_state: None,
        }
    }
    pub fn ok_empty() -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: None,
            command_state: None,
        }
    }
    pub fn err(msg: String) -> Self {
        Self {
            ok: false,
            output: None,
            tabs: None,
            error: Some(msg),
            tab_index: None,
            command_state: None,
        }
    }
    pub fn ok_status(state: String) -> Self {
        Self {
            ok: true,
            output: None,
            tabs: None,
            error: None,
            tab_index: None,
            command_state: Some(state),
        }
    }
}

/// A pending socket request paired with a oneshot channel for the response.
pub struct PendingSocketRequest {
    pub request: SocketRequest,
    pub reply_tx: tokio::sync::oneshot::Sender<SocketResponse>,
}

// ─── Socket path helper ─────────────────────────────────────────────────────

/// Compute the Unix socket path for a given project path.
/// Format: /tmp/opman-{hash}.sock
pub fn socket_path_for_project(project_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    project_path.hash(&mut hasher);
    let hash = hasher.finish();
    PathBuf::from(format!("/tmp/opman-{:x}.sock", hash))
}

// ─── Unix socket server (runs inside the manager process) ───────────────────

/// Spawn the Unix domain socket server for a single project.
/// Each incoming connection reads one JSON line, sends a PendingSocketRequest
/// through `request_tx`, waits for the response, and writes it back.
///
/// Concurrency controls:
/// - Ephemeral task names are deduplicated (only one in-flight per name).
/// - Neovim operations are serialized per-file (different files run concurrently).
/// - Terminal tab busy state is checked at the application level in `handle_mcp_request`.
///
/// Neovim operations are handled **directly** in the socket server task when the
/// neovim socket is already registered (i.e. neovim has been spawned). This avoids
/// round-tripping through the main TUI event loop for blocking `nvim_rpc` calls.
/// If neovim hasn't been spawned yet, the request falls through to the main loop
/// which lazy-spawns neovim and registers the socket path.
pub fn spawn_socket_server(
    project_path: &Path,
    request_tx: mpsc::UnboundedSender<crate::app::BackgroundEvent>,
    project_idx: usize,
    nvim_registry: NvimSocketRegistry,
) -> PathBuf {
    let sock_path = socket_path_for_project(project_path);

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
        // Used by ephemeral_lock/ephemeral_unlock ops from the MCP bridge.
        let busy_ephemeral: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        // Per-file lock map for neovim operations. Operations on different
        // files run concurrently; operations on the same file (or with no
        // file_path, keyed as "__current__") are serialized.
        let nvim_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Per-tab lock map for terminal operations. Operations on different
        // tabs run concurrently; operations on the same tab are serialized.
        // Key: tab index (as string), or "__no_tab__" for ops without a tab.
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

            tokio::spawn(async move {
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

                // Handle ephemeral_lock / ephemeral_unlock directly (no main-loop round-trip)
                match request.op.as_str() {
                    "ephemeral_lock" => {
                        let name = match &request.name {
                            Some(n) => n.clone(),
                            None => {
                                let resp =
                                    SocketResponse::err("Missing 'name' for ephemeral_lock".into());
                                let _ = writer
                                    .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
                                    .await;
                                let _ = writer.write_all(b"\n").await;
                                return;
                            }
                        };
                        let acquired = {
                            let mut names = eph.lock().unwrap();
                            names.insert(name.clone())
                        };
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

                // Acquire per-file neovim lock(s) for nvim_* ops. Operations on
                // different files run concurrently; same-file ops are serialized.
                // Multi-edit batches acquire locks for ALL touched files (sorted
                // to avoid deadlocks).
                let is_nvim_op = request.op.starts_with("nvim_");
                let nvim_lock_keys: Vec<String> = if is_nvim_op {
                    if let Some(ref edits) = request.edits {
                        // Multi-edit: collect unique file paths, sorted for
                        // consistent lock ordering (prevents deadlocks).
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
                // Acquire per-file locks. Keep Arcs alive so guards can borrow them.
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
                // Acquire all locks in order (sorted keys → deterministic order).
                let mut _nvim_guards = Vec::new();
                for arc in &nvim_arcs {
                    _nvim_guards.push(arc.lock().await);
                }

                // Acquire per-tab terminal lock for terminal ops that target a
                // specific tab. Operations on different tabs run concurrently;
                // same-tab ops are serialized.
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

                // For nvim_* ops: try to handle directly using the registry
                // (avoids round-trip through the main TUI event loop).
                if is_nvim_op {
                    let session_id = request.session_id.clone().unwrap_or_default();
                    let nvim_socket = {
                        let reg = registry.read().await;
                        reg.get(&(pidx, session_id.clone())).cloned()
                    };

                    if let Some(nvim_socket) = nvim_socket {
                        // Handle nvim operation directly in this task using
                        // spawn_blocking (nvim_rpc uses synchronous I/O).
                        let response = {
                            let req = request;
                            let sock = nvim_socket;
                            tokio::task::spawn_blocking(move || {
                                handle_nvim_op_blocking(&sock, &req)
                            })
                            .await
                            .unwrap_or_else(|e| {
                                SocketResponse::err(format!("Task join error: {}", e))
                            })
                        };

                        let json = serde_json::to_string(&response).unwrap();
                        let _ = writer.write_all(json.as_bytes()).await;
                        let _ = writer.write_all(b"\n").await;

                        drop(_nvim_guards);
                        return;
                    }
                    // else: neovim not yet spawned — fall through to main loop
                    // which will lazy-spawn it and register the socket.
                }

                // Send to main event loop and wait for response
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                // Extract session_id from the request (set by bridge from
                // OPENCODE_SESSION_ID env var) before wrapping.
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

                // Release locks (dropped when guards go out of scope)
                drop(_nvim_guards);
                drop(_term_guard);
            });
        }
    });

    sock_path
}

/// Handle a neovim operation directly (blocking I/O via nvim_rpc).
///
/// This is called from `spawn_blocking` in the socket server task, bypassing
/// the main TUI event loop entirely. All nvim_rpc functions use synchronous
/// UnixStream I/O, so they must run on a blocking thread.
fn handle_nvim_op_blocking(nvim_socket: &Path, request: &SocketRequest) -> SocketResponse {
    // Resolve file_path → buffer handle (0 = current buffer)
    let buf: i64 = if let Some(ref path) = request.file_path {
        // nvim_open handles its own file_path, skip resolution
        if request.op != "nvim_open" {
            match crate::nvim_rpc::nvim_find_or_load_buffer(nvim_socket, path) {
                Ok(id) => id,
                Err(e) => {
                    return SocketResponse::err(format!(
                        "Failed to resolve buffer for '{}': {}",
                        path, e
                    ))
                }
            }
        } else {
            0
        }
    } else {
        0 // current buffer
    };

    match request.op.as_str() {
        "nvim_open" => {
            let file_path = match &request.file_path {
                Some(p) => p.as_str(),
                None => return SocketResponse::err("Missing 'file_path' for nvim_open".into()),
            };
            match crate::nvim_rpc::nvim_open_file(nvim_socket, file_path, request.line) {
                Ok(()) => {
                    let mut msg = format!("Opened {}", file_path);
                    if let Some(ln) = request.line {
                        msg.push_str(&format!(" at line {}", ln));
                    }
                    SocketResponse::ok_text(msg)
                }
                Err(e) => SocketResponse::err(format!("Failed to open file in Neovim: {}", e)),
            }
        }
        "nvim_read" => {
            // Convert from 1-indexed (user-facing) to 0-indexed (nvim API)
            let start = request.line.unwrap_or(1).max(1) - 1;
            let end = match request.end_line {
                Some(-1) | None => match crate::nvim_rpc::nvim_buf_line_count(nvim_socket, buf) {
                    Ok(count) => count,
                    Err(e) => {
                        return SocketResponse::err(format!("Failed to get line count: {}", e))
                    }
                },
                Some(e) => e,
            };
            let lang = crate::nvim_rpc::nvim_buf_get_name(nvim_socket, buf)
                .map(|name| crate::mcp_neovim::ext_to_lang(&name).to_string())
                .unwrap_or_default();

            match crate::nvim_rpc::nvim_buf_get_lines(nvim_socket, buf, start, end) {
                Ok(lines) => {
                    let numbered: Vec<String> = lines
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{}: {}", start + 1 + i as i64, l))
                        .collect();
                    let body = numbered.join("\n");
                    SocketResponse::ok_text(format!("```{}\n{}\n```", lang, body))
                }
                Err(e) => SocketResponse::err(format!("Failed to read lines from Neovim: {}", e)),
            }
        }
        "nvim_command" => {
            let cmd = match &request.command {
                Some(c) => c.as_str(),
                None => return SocketResponse::err("Missing 'command' for nvim_command".into()),
            };
            match crate::nvim_rpc::nvim_command(nvim_socket, cmd) {
                Ok(()) => SocketResponse::ok_text(format!("Command executed: {}", cmd)),
                Err(e) => SocketResponse::err(format!("Neovim command failed: {}", e)),
            }
        }
        "nvim_buffers" => match crate::nvim_rpc::nvim_list_bufs(nvim_socket) {
            Ok(bufs) => {
                if bufs.is_empty() {
                    SocketResponse::ok_text("No named buffers loaded.".into())
                } else {
                    let lines: Vec<String> = bufs
                        .iter()
                        .map(|(id, name)| format!("Buffer {}: {}", id, name))
                        .collect();
                    SocketResponse::ok_text(lines.join("\n"))
                }
            }
            Err(e) => SocketResponse::err(format!("Failed to list buffers: {}", e)),
        },
        "nvim_info" => {
            let name = crate::nvim_rpc::nvim_buf_get_name(nvim_socket, buf)
                .unwrap_or_else(|_| "(unknown)".into());
            let cursor = crate::nvim_rpc::nvim_cursor_pos(nvim_socket).unwrap_or((1, 0));
            let line_count = crate::nvim_rpc::nvim_buf_line_count(nvim_socket, buf).unwrap_or(0);

            let info = format!(
                "Buffer: {}\nCursor: line {}, column {}\nTotal lines: {}",
                if name.is_empty() { "(unnamed)" } else { &name },
                cursor.0,
                cursor.1,
                line_count
            );
            SocketResponse::ok_text(info)
        }
        "nvim_diagnostics" => {
            let buf_only = request.buf_only.unwrap_or(false);
            match crate::nvim_rpc::nvim_lsp_diagnostics(nvim_socket, buf, buf_only) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get diagnostics: {}", e)),
            }
        }
        "nvim_definition" => {
            match crate::nvim_rpc::nvim_lsp_definition(nvim_socket, buf, request.line, request.col)
            {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get definition: {}", e)),
            }
        }
        "nvim_references" => {
            match crate::nvim_rpc::nvim_lsp_references(nvim_socket, buf, request.line, request.col)
            {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get references: {}", e)),
            }
        }
        "nvim_hover" => {
            match crate::nvim_rpc::nvim_lsp_hover(nvim_socket, buf, request.line, request.col) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get hover info: {}", e)),
            }
        }
        "nvim_symbols" => {
            let query = request.query.as_deref().unwrap_or("");
            let workspace = request.workspace.unwrap_or(false);
            match crate::nvim_rpc::nvim_lsp_symbols(nvim_socket, buf, query, workspace) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to get symbols: {}", e)),
            }
        }
        "nvim_code_actions" => match crate::nvim_rpc::nvim_lsp_code_actions(nvim_socket, buf) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(e) => SocketResponse::err(format!("Failed to get code actions: {}", e)),
        },
        "nvim_eval" => {
            let code = match &request.command {
                Some(c) => c.as_str(),
                None => {
                    return SocketResponse::err("Missing 'command' (Lua code) for nvim_eval".into())
                }
            };
            match crate::nvim_rpc::nvim_eval_lua(nvim_socket, code) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Lua eval failed: {}", e)),
            }
        }
        "nvim_grep" => {
            let pattern = match &request.query {
                Some(q) => q.as_str(),
                None => {
                    return SocketResponse::err(
                        "Missing 'query' (search pattern) for nvim_grep".into(),
                    )
                }
            };
            let glob = request.glob.as_deref();
            match crate::nvim_rpc::nvim_grep(nvim_socket, pattern, glob) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Grep failed: {}", e)),
            }
        }
        "nvim_diff" => match crate::nvim_rpc::nvim_buf_diff(nvim_socket, buf) {
            Ok(output) => {
                if output.is_empty() {
                    SocketResponse::ok_text("No unsaved changes.".into())
                } else {
                    SocketResponse::ok_text(output)
                }
            }
            Err(e) => SocketResponse::err(format!("Failed to compute diff: {}", e)),
        },
        "nvim_write" => {
            let all = request.all.unwrap_or(false);
            match crate::nvim_rpc::nvim_write(nvim_socket, buf, all) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Failed to write: {}", e)),
            }
        }
        "nvim_edit_and_save" => {
            if let Some(edit_ops) = &request.edits {
                // Multi-edit batch path
                let mut resolved: Vec<crate::nvim_rpc::ResolvedEdit> = Vec::new();
                for (i, op) in edit_ops.iter().enumerate() {
                    let edit_buf =
                        match crate::nvim_rpc::nvim_find_or_load_buffer(nvim_socket, &op.file_path)
                        {
                            Ok(id) => id,
                            Err(e) => {
                                return SocketResponse::err(format!(
                                    "edits[{}]: failed to resolve buffer for '{}': {}",
                                    i, op.file_path, e
                                ))
                            }
                        };
                    resolved.push(crate::nvim_rpc::ResolvedEdit {
                        buf: edit_buf,
                        file_path: op.file_path.clone(),
                        start_line: op.start_line,
                        end_line: op.end_line,
                        new_text: op.new_text.clone(),
                    });
                }
                match crate::nvim_rpc::nvim_buf_multi_edit_and_save(nvim_socket, &mut resolved) {
                    Ok(msg) => SocketResponse::ok_text(msg),
                    Err(e) => SocketResponse::err(format!("Multi-edit failed: {}", e)),
                }
            } else {
                // Single edit path
                let start_line = match request.line {
                    Some(l) => l,
                    None => {
                        return SocketResponse::err(
                            "Missing 'start_line' for nvim_edit_and_save".into(),
                        )
                    }
                };
                let end_line = match request.end_line {
                    Some(l) => l,
                    None => {
                        return SocketResponse::err(
                            "Missing 'end_line' for nvim_edit_and_save".into(),
                        )
                    }
                };
                let new_text = match &request.new_text {
                    Some(t) => t.as_str(),
                    None => {
                        return SocketResponse::err(
                            "Missing 'new_text' for nvim_edit_and_save".into(),
                        )
                    }
                };
                match crate::nvim_rpc::nvim_buf_set_text_and_save(
                    nvim_socket,
                    buf,
                    start_line,
                    end_line,
                    new_text,
                ) {
                    Ok(msg) => SocketResponse::ok_text(msg),
                    Err(e) => SocketResponse::err(format!("Edit+save failed: {}", e)),
                }
            }
        }
        "nvim_undo" => {
            let count = request.count.unwrap_or(1);
            match crate::nvim_rpc::nvim_undo(nvim_socket, buf, count) {
                Ok(msg) => SocketResponse::ok_text(msg),
                Err(e) => SocketResponse::err(format!("Undo failed: {}", e)),
            }
        }
        "nvim_rename" => {
            let new_name = match &request.new_name {
                Some(n) => n.as_str(),
                None => return SocketResponse::err("Missing 'new_name' for nvim_rename".into()),
            };
            match crate::nvim_rpc::nvim_lsp_rename(
                nvim_socket,
                buf,
                new_name,
                request.line,
                request.col,
            ) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Rename failed: {}", e)),
            }
        }
        "nvim_format" => match crate::nvim_rpc::nvim_lsp_format(nvim_socket, buf) {
            Ok(output) => SocketResponse::ok_text(output),
            Err(e) => SocketResponse::err(format!("Format failed: {}", e)),
        },
        "nvim_signature" => {
            match crate::nvim_rpc::nvim_lsp_signature(nvim_socket, buf, request.line, request.col) {
                Ok(output) => SocketResponse::ok_text(output),
                Err(e) => SocketResponse::err(format!("Signature help failed: {}", e)),
            }
        }
        _ => SocketResponse::err(format!("Unknown nvim operation: {}", request.op)),
    }
}

// ─── Cleanup: remove socket files on shutdown ───────────────────────────────

pub fn cleanup_socket(project_path: &Path) {
    let sock = socket_path_for_project(project_path);
    let _ = std::fs::remove_file(&sock);
}

// ─── MCP stdio bridge (runs as `opman --mcp <project_path>`) ─────

/// MCP JSON-RPC request (subset we handle).
#[derive(Debug, Deserialize)]
struct McpJsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
    id: serde_json::Value,
}

/// Run the MCP stdio bridge: read JSON-RPC from stdin, forward to socket, write response to stdout.
///
/// Tool calls (`tools/call`) are dispatched **concurrently**: each call is
/// spawned as a separate tokio task, and responses are written back with the
/// correct `id` as they complete — not in request order.  This allows the MCP
/// client (e.g. opencode) to issue parallel tool calls that execute at the same
/// time (subject to per-resource locking in the socket server layer).
///
/// Non-tool methods (initialize, tools/list) are still handled inline since
/// they are cheap and ordering-sensitive (initialize must complete before
/// tools/list).
///
/// This function is designed to **never exit** on transient errors. Only a
/// genuine EOF on stdin (the MCP client closed the pipe) causes a clean exit.
/// All stdout write failures are swallowed — the individual request is lost
/// but the bridge stays alive so subsequent requests still work.
pub async fn run_mcp_bridge(project_path: PathBuf) -> anyhow::Result<()> {
    let sock_path = Arc::new(socket_path_for_project(&project_path));
    // Read session ID from env var set by opencode PTY spawn, so all
    // socket requests route to the correct per-session resources.
    let session_id: Arc<Option<String>> = Arc::new(std::env::var("OPENCODE_SESSION_ID").ok());
    let stdin = tokio::io::stdin();
    // Shared stdout writer protected by a tokio Mutex so concurrent tasks can
    // write responses without interleaving.
    let stdout: Arc<tokio::sync::Mutex<tokio::io::Stdout>> =
        Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    let mut reader = BufReader::new(stdin);

    let mut line = String::new();
    loop {
        line.clear();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => {
                eprintln!("MCP bridge stdin read error: {}", e);
                continue;
            }
        };
        if n == 0 {
            break; // EOF — client closed the pipe
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let rpc_req: McpJsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let error_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) },
                    "id": null
                });
                write_jsonrpc_stdout(&stdout, &error_resp).await;
                continue;
            }
        };

        let _ = rpc_req.jsonrpc; // consumed for deserialization

        match rpc_req.method.as_str() {
            "initialize" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "opman-terminal",
                            "version": "1.0.0"
                        }
                    },
                    "id": rpc_req.id
                });
                write_jsonrpc_stdout(&stdout, &response).await;
            }
            "notifications/initialized" => {
                // Client acknowledgment, no response needed
                continue;
            }
            "tools/list" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "tools": mcp_tool_definitions()
                    },
                    "id": rpc_req.id
                });
                write_jsonrpc_stdout(&stdout, &response).await;
            }
            "tools/call" => {
                // Spawn tool call concurrently — does not block the stdin reader
                let sock = Arc::clone(&sock_path);
                let sid = Arc::clone(&session_id);
                let out = Arc::clone(&stdout);
                let id = rpc_req.id.clone();
                let params = rpc_req.params;
                tokio::spawn(async move {
                    let result = handle_tool_call(&sock, params, sid.as_deref()).await;
                    let response = match result {
                        Ok(content) => {
                            serde_json::json!({
                                "jsonrpc": "2.0",
                                "result": {
                                    "content": content
                                },
                                "id": id
                            })
                        }
                        Err(e) => {
                            serde_json::json!({
                                "jsonrpc": "2.0",
                                "result": {
                                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                                    "isError": true
                                },
                                "id": id
                            })
                        }
                    };
                    write_jsonrpc_stdout(&out, &response).await;
                });
            }
            _ => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32601, "message": format!("Method not found: {}", rpc_req.method) },
                    "id": rpc_req.id
                });
                write_jsonrpc_stdout(&stdout, &response).await;
            }
        }
    }

    Ok(())
}

/// Write a JSON-RPC response to a shared stdout (tokio Mutex-protected).
/// The entire write (json + newline + flush) is atomic w.r.t. the lock so
/// concurrent tasks never interleave their output.
async fn write_jsonrpc_stdout(
    stdout: &Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    resp: &serde_json::Value,
) {
    let json = match serde_json::to_string(resp) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("MCP bridge: failed to serialize response: {}", e);
            return;
        }
    };
    let mut out = stdout.lock().await;
    if let Err(e) = out.write_all(json.as_bytes()).await {
        eprintln!("MCP bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.write_all(b"\n").await {
        eprintln!("MCP bridge: stdout write error: {}", e);
        return;
    }
    if let Err(e) = out.flush().await {
        eprintln!("MCP bridge: stdout flush error: {}", e);
    }
}

/// Return MCP tool definitions for tools/list.
fn mcp_tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "terminal_read",
            "description": "Read the terminal output from a terminal tab in the opman. Returns the full terminal buffer (scrollback + visible). Use last_n to limit to the most recent N lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based). Defaults to the active tab if not specified."
                    },
                    "last_n": {
                        "type": "number",
                        "description": "Return only the last N lines of the terminal output."
                    }
                }
            }
        },
        {
            "name": "terminal_run",
            "description": "Run a command in a terminal tab in the opman. The command is typed into the terminal and executed. Use this to run shell commands, scripts, or interact with running processes. If a command is already running on the tab, this will return an error — send Ctrl-C (\\x03) as the command to interrupt it first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run in the terminal. Send \"\\x03\" (Ctrl-C) to interrupt a running command."
                    },
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based). Defaults to the active tab if not specified."
                    },
                    "wait": {
                        "type": "boolean",
                        "description": "If true, wait for command output to settle and return the terminal screen content. If false (default), fire-and-forget — returns immediately, use terminal_read to check output later."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait for command to complete when wait=true (default: 30)."
                    }
                },
                "required": ["command", "tab"]
            }
        },
        {
            "name": "terminal_list",
            "description": "List all terminal tabs in the opman for the current project. Returns tab indices and which tab is currently active.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "terminal_new",
            "description": "Create a new terminal tab in the opman. Returns the index of the newly created tab.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Optional name for the new tab"
                    }
                }
            }
        },
        {
            "name": "terminal_close",
            "description": "Close a terminal tab in the opman.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based) to close. Defaults to the active tab if not specified."
                    }
                }
            }
        },
        {
            "name": "terminal_rename",
            "description": "Rename a terminal tab",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tab": {
                        "type": "number",
                        "description": "Tab index (0-based) to rename"
                    },
                    "name": {
                        "type": "string",
                        "description": "New name for the tab"
                    }
                },
                "required": ["tab", "name"]
            }
        },
        {
            "name": "terminal_ephemeral_run",
            "description": "Run a command in a named ephemeral terminal tab. Creates a temporary tab (or reuses one with the same name), runs the command, waits for completion, returns the output, and closes the tab.\n\nUse a unique `name` for each independent task you want to run in PARALLEL (e.g. \"build\", \"test\", \"lint\"). Two calls with the same name cannot run concurrently — the second will be rejected. Use the SAME name for commands that must run SEQUENTIALLY on the same logical task.\n\nThis is the PREFERRED tool for one-shot commands — use this instead of terminal_run when you just need command output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run in the terminal."
                    },
                    "name": {
                        "type": "string",
                        "description": "A logical name for this task (e.g. \"build\", \"test\", \"lint\"). Use different names to run commands in parallel. Use the same name for sequential commands that belong to the same task — a second parallel call with the same name will be rejected."
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Maximum time in seconds to wait for command output to settle (default: 30). The tool polls until output stabilizes or timeout is reached."
                    }
                },
                "required": ["command", "name"]
            }
        }
    ])
}

/// Handle a tools/call request by forwarding to the Unix socket.
async fn handle_tool_call(
    sock_path: &Path,
    params: Option<serde_json::Value>,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let params = params.unwrap_or(serde_json::json!({}));
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Handle ephemeral run as a composite operation (new → run → poll → close)
    if tool_name == "terminal_ephemeral_run" {
        return handle_ephemeral_run(sock_path, &arguments, session_id).await;
    }

    // Build internal socket request
    let socket_req = match tool_name {
        "terminal_read" => SocketRequest {
            op: "read".into(),
            tab: arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            last_n: arguments
                .get("last_n")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            ..Default::default()
        },
        "terminal_run" => {
            let cmd = arguments
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("terminal_run requires 'command' argument"))?;
            let wait = arguments.get("wait").and_then(|v| v.as_bool());
            SocketRequest {
                op: "run".into(),
                tab: arguments
                    .get("tab")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                command: Some(cmd.to_string()),
                wait,
                ..Default::default()
            }
        }
        "terminal_list" => SocketRequest {
            op: "list".into(),
            ..Default::default()
        },
        "terminal_new" => SocketRequest {
            op: "new".into(),
            name: arguments
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        },
        "terminal_close" => SocketRequest {
            op: "close".into(),
            tab: arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            ..Default::default()
        },
        "terminal_rename" => {
            let tab = arguments
                .get("tab")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| anyhow::anyhow!("terminal_rename requires 'tab' argument"))?;
            let name = arguments
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("terminal_rename requires 'name' argument"))?;
            SocketRequest {
                op: "rename".into(),
                tab: Some(tab),
                name: Some(name.to_string()),
                ..Default::default()
            }
        }
        other => {
            return Ok(serde_json::json!([{
                "type": "text",
                "text": format!("Unknown tool: {}", other)
            }]));
        }
    };

    // Inject session_id into the request for correct routing
    let mut socket_req = socket_req;
    socket_req.session_id = session_id.map(|s| s.to_string());

    // Check if this is a "run" with wait — we'll need the tab and wait flag before sending
    let is_wait_run = tool_name == "terminal_run" && socket_req.wait.unwrap_or(false);
    let wait_tab = socket_req.tab;
    let wait_timeout_secs = if is_wait_run {
        arguments
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
    } else {
        30
    };

    // Send the primary request
    let socket_resp = send_socket_request(sock_path, &socket_req).await?;

    // For "run" with wait=true: poll command_state until command finishes or timeout
    if is_wait_run && socket_resp.ok {
        let timed_out =
            poll_command_completion(sock_path, wait_tab, wait_timeout_secs, session_id).await;

        let read_req = SocketRequest {
            op: "read".into(),
            tab: wait_tab,
            session_id: session_id.map(|s| s.to_string()),
            ..Default::default()
        };
        let read_resp = send_socket_request(sock_path, &read_req).await?;

        if timed_out {
            let mut output = read_resp.output.unwrap_or_default();
            output = format!("[TIMEOUT after {}s]\n{}", wait_timeout_secs, output);
            return Ok(serde_json::json!([{
                "type": "text",
                "text": output
            }]));
        }

        return format_mcp_response(&read_resp);
    }

    format_mcp_response(&socket_resp)
}

async fn handle_ephemeral_run(
    sock_path: &Path,
    arguments: &serde_json::Value,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let cmd = arguments
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("terminal_ephemeral_run requires 'command' argument"))?;

    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("terminal_ephemeral_run requires 'name' argument"))?;

    let timeout_secs = arguments
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let sid = session_id.map(|s| s.to_string());

    // 0. Acquire ephemeral name lock — rejects if same name is already running
    let lock_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "ephemeral_lock".into(),
            name: Some(name.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await?;
    if !lock_resp.ok {
        let msg = lock_resp
            .error
            .unwrap_or_else(|| "Ephemeral lock failed".into());
        return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
    }

    // From here on, we must unlock on every exit path
    let result =
        handle_ephemeral_run_inner(sock_path, cmd, name, timeout_secs, sid.as_deref()).await;

    // Always release the ephemeral name lock
    let _ = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "ephemeral_unlock".into(),
            name: Some(name.to_string()),
            session_id: sid,
            ..Default::default()
        },
    )
    .await;

    result
}

async fn handle_ephemeral_run_inner(
    sock_path: &Path,
    cmd: &str,
    name: &str,
    timeout_secs: u64,
    session_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let sid = session_id.map(|s| s.to_string());

    // 1. Create ephemeral tab
    let new_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "new".into(),
            name: Some(name.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await?;

    let tab_idx = match new_resp.tab_index {
        Some(idx) => idx,
        None => {
            let msg = new_resp
                .error
                .unwrap_or_else(|| "Failed to create tab".into());
            return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
        }
    };

    // 2. Run the command in the ephemeral tab
    let run_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "run".into(),
            tab: Some(tab_idx),
            command: Some(cmd.to_string()),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await;

    if let Err(e) = &run_resp {
        let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;
        return Err(anyhow::anyhow!("Failed to run command: {}", e));
    }
    if !run_resp.as_ref().unwrap().ok {
        let msg = run_resp
            .unwrap()
            .error
            .unwrap_or_else(|| "Run failed".into());
        let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;
        return Ok(serde_json::json!([{ "type": "text", "text": msg }]));
    }

    let timed_out =
        poll_command_completion(sock_path, Some(tab_idx), timeout_secs, sid.as_deref()).await;

    let read_resp = send_socket_request(
        sock_path,
        &SocketRequest {
            op: "read".into(),
            tab: Some(tab_idx),
            session_id: sid.clone(),
            ..Default::default()
        },
    )
    .await;

    let mut final_output = match read_resp {
        Ok(ref r) if r.ok => r.output.clone().unwrap_or_default(),
        _ => String::new(),
    };

    if timed_out {
        final_output = format!("[TIMEOUT after {}s]\n{}", timeout_secs, final_output);
    }

    // 3. Close the ephemeral tab
    let _ = close_tab(sock_path, tab_idx, sid.as_deref()).await;

    Ok(serde_json::json!([{
        "type": "text",
        "text": final_output
    }]))
}

/// Poll command_state until the command finishes or timeout expires.
/// Returns true if timed out, false if command completed.
///
/// Two-phase polling:
/// 1. Wait for state to become "running" (shell started processing the command)
/// 2. Wait for state to leave "running" (command finished)
///
/// This avoids the race where we poll before the shell processes the command
/// and see a stale "idle"/"success"/"failure" from the previous command.
async fn poll_command_completion(
    sock_path: &Path,
    tab: Option<usize>,
    timeout_secs: u64,
    session_id: Option<&str>,
) -> bool {
    let poll_interval = std::time::Duration::from_millis(300);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    let status_req = SocketRequest {
        op: "status".into(),
        tab,
        session_id: session_id.map(|s| s.to_string()),
        ..Default::default()
    };

    // Phase 1: wait for state to become "running"
    loop {
        if std::time::Instant::now() >= deadline {
            return true;
        }

        tokio::time::sleep(poll_interval).await;

        let state = match send_socket_request(sock_path, &status_req).await {
            Ok(ref r) if r.ok => r.command_state.clone().unwrap_or_default(),
            _ => return false,
        };

        if state == "running" {
            break;
        }
    }

    // Phase 2: wait for state to leave "running"
    loop {
        if std::time::Instant::now() >= deadline {
            return true;
        }

        tokio::time::sleep(poll_interval).await;

        let state = match send_socket_request(sock_path, &status_req).await {
            Ok(ref r) if r.ok => r.command_state.clone().unwrap_or_default(),
            _ => return false,
        };

        match state.as_str() {
            "running" => continue,
            _ => return false,
        }
    }
}

async fn close_tab(
    sock_path: &Path,
    tab_idx: usize,
    session_id: Option<&str>,
) -> anyhow::Result<SocketResponse> {
    send_socket_request(
        sock_path,
        &SocketRequest {
            op: "close".into(),
            tab: Some(tab_idx),
            session_id: session_id.map(|s| s.to_string()),
            ..Default::default()
        },
    )
    .await
}

/// Send a SocketRequest over the Unix socket and return the response.
async fn send_socket_request(
    sock_path: &Path,
    request: &SocketRequest,
) -> anyhow::Result<SocketResponse> {
    let mut stream = tokio::net::UnixStream::connect(sock_path)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to connect to manager socket at {:?}: {}. Is opman running?",
                sock_path,
                e
            )
        })?;

    let req_json = serde_json::to_string(request)?;
    stream.write_all(req_json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    // Shutdown write side so the server knows we're done sending
    stream.shutdown().await?;

    // Read response
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await?;
    let resp_str = String::from_utf8_lossy(&resp_buf);

    serde_json::from_str(resp_str.trim())
        .map_err(|e| anyhow::anyhow!("Invalid response from manager: {}", e))
}

/// Convert a SocketResponse to MCP content format.
fn format_mcp_response(socket_resp: &SocketResponse) -> anyhow::Result<serde_json::Value> {
    if !socket_resp.ok {
        let error_msg = socket_resp.error.as_deref().unwrap_or("Unknown error");
        return Ok(serde_json::json!([{
            "type": "text",
            "text": error_msg
        }]));
    }

    if let Some(ref output) = socket_resp.output {
        Ok(serde_json::json!([{
            "type": "text",
            "text": output
        }]))
    } else if let Some(ref tabs) = socket_resp.tabs {
        let tab_text = tabs
            .iter()
            .map(|t| {
                let name_part = if t.name.is_empty() {
                    String::new()
                } else {
                    format!(" \"{}\"", t.name)
                };
                let active_part = if t.active { " (active)" } else { "" };
                format!("Tab {}{}{}", t.index, name_part, active_part)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(serde_json::json!([{
            "type": "text",
            "text": tab_text
        }]))
    } else if let Some(tab_index) = socket_resp.tab_index {
        Ok(serde_json::json!([{
            "type": "text",
            "text": format!("Created new terminal tab at index {}", tab_index)
        }]))
    } else {
        Ok(serde_json::json!([{
            "type": "text",
            "text": "OK"
        }]))
    }
}

// ─── opencode.json auto-generation ──────────────────────────────────────────

/// Write (or update) the opencode.json file for a project to include the MCP server configs.
pub fn write_opencode_json(
    project_path: &Path,
    enable_terminal: bool,
    enable_neovim: bool,
    enable_time: bool,
) -> anyhow::Result<()> {
    let json_path = project_path.join("opencode.json");

    // Read existing config or start fresh
    let mut config: serde_json::Value = if json_path.exists() {
        let content = std::fs::read_to_string(&json_path)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Get the current executable path for the MCP command
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman"));
    let exe_str = exe_path.to_string_lossy().to_string();

    let project_path_str = project_path.to_string_lossy().to_string();

    // Set mcp.* configs based on enabled flags
    let mcp = config
        .as_object_mut()
        .map(|obj| obj.entry("mcp").or_insert(serde_json::json!({})))
        .unwrap();

    if let Some(mcp_obj) = mcp.as_object_mut() {
        if enable_terminal {
            mcp_obj.insert(
                "terminal".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "--mcp", &project_path_str]
                }),
            );
        } else {
            mcp_obj.remove("terminal");
        }
        if enable_neovim {
            mcp_obj.insert(
                "neovim".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "--mcp-nvim", &project_path_str]
                }),
            );
        } else {
            mcp_obj.remove("neovim");
        }
        if enable_time {
            mcp_obj.insert(
                "time".to_string(),
                serde_json::json!({
                    "type": "local",
                    "command": [&exe_str, "--mcp-time"]
                }),
            );
        } else {
            mcp_obj.remove("time");
        }
    }

    // Disable opencode's native bash tool so it uses the manager's terminal instead
    if enable_terminal {
        let permission = config
            .as_object_mut()
            .map(|obj| obj.entry("permission").or_insert(serde_json::json!({})))
            .unwrap();
        if let Some(perm_obj) = permission.as_object_mut() {
            perm_obj.insert("bash".to_string(), serde_json::json!("deny"));
        }
    }

    // When neovim MCP is enabled, disable opencode's native edit/write tools
    // since the AI edits files through neovim directly.
    if enable_neovim {
        let permission = config
            .as_object_mut()
            .map(|obj| obj.entry("permission").or_insert(serde_json::json!({})))
            .unwrap();
        if let Some(perm_obj) = permission.as_object_mut() {
            perm_obj.insert("edit".to_string(), serde_json::json!("deny"));
        }
    }

    let formatted = serde_json::to_string_pretty(&config)?;
    std::fs::write(&json_path, formatted)?;
    info!(
        ?json_path,
        enable_terminal, enable_neovim, enable_time, "Wrote opencode.json with MCP config"
    );

    Ok(())
}
