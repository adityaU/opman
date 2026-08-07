//! One language server process, from spawn to shutdown.
//!
//! Startup is the interesting part. `initialize` can take rust-analyzer several
//! seconds, and meanwhile more requests arrive for the same file. Making them
//! queue behind a lock would serialise everything; letting each one initialise
//! would start several servers. Instead the handshake is a single shared future
//! every caller awaits: the first request drives it, the rest join it, and
//! `ensure` never holds a lock across it.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures::future::{BoxFuture, FutureExt, Shared};
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::debug;

use super::convert::{path_to_uri, PositionEncoding};
use super::detect::ServerSpec;
use super::diags::DiagStore;
use super::docs::DocStore;
use super::notify::ServerHandler;
use super::peer::Peer;

/// Every `CompletionItemKind` in the spec, so servers do not downgrade to the
/// 1.0 subset and lose their icons.
const COMPLETION_KINDS: [i32; 25] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
];

pub const INIT_TIMEOUT: Duration = Duration::from_secs(20);
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(6);
pub const FORMAT_TIMEOUT: Duration = Duration::from_secs(15);

/// What the server said it can do. A capability it never claimed is one we must
/// not offer, so the editor shows the feature as unavailable rather than
/// silently returning nothing.
#[derive(Clone, Default)]
pub struct ServerCaps {
    pub hover: bool,
    pub definition: bool,
    pub formatting: bool,
    pub completion: bool,
    /// Characters that should re-query completions mid-word — `.` and `:` for
    /// Rust, `.` and `"` for JSON. The editor cannot guess these per language.
    pub trigger_characters: Vec<String>,
    pub encoding: PositionEncoding,
}

type Ready = Shared<BoxFuture<'static, Result<ServerCaps, Arc<anyhow::Error>>>>;

pub struct LspServer {
    pub peer: Peer,
    pub docs: Arc<DocStore>,
    pub diags: Arc<DiagStore>,
    pub root: PathBuf,
    child: Mutex<Child>,
    ready: Ready,
    last_used: AtomicU64,
}

impl LspServer {
    /// Spawn the server and send `initialize`. Returns as soon as the process
    /// exists — the handshake completes in the background.
    pub fn spawn(spec: &ServerSpec, binary: &Path, root: &Path) -> Result<Arc<Self>> {
        let mut child = Command::new(binary)
            .args(spec.args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to start {}", spec.command))?;

        let stdin = child.stdin.take().context("language server stdin missing")?;
        let stdout = child
            .stdout
            .take()
            .context("language server stdout missing")?;

        let diags = Arc::new(DiagStore::new());
        let peer = Peer::new(
            (stdout, stdin),
            Arc::new(ServerHandler {
                diags: diags.clone(),
            }),
        );

        let handshake = handshake(peer.clone(), root.to_path_buf());
        Ok(Arc::new(Self {
            peer,
            docs: Arc::new(DocStore::new()),
            diags,
            root: root.to_path_buf(),
            child: Mutex::new(child),
            ready: handshake.boxed().shared(),
            last_used: AtomicU64::new(now_secs()),
        }))
    }

    /// Await the handshake. Cheap and idempotent after the first call.
    pub async fn ready(&self) -> Result<ServerCaps> {
        match tokio::time::timeout(INIT_TIMEOUT, self.ready.clone()).await {
            Ok(Ok(caps)) => Ok(caps),
            Ok(Err(e)) => bail!("language server failed to start: {e}"),
            Err(_) => bail!("language server did not finish initializing"),
        }
    }

    pub fn touch(&self) {
        self.last_used.store(now_secs(), Ordering::Relaxed);
    }

    pub fn idle_for(&self) -> Duration {
        Duration::from_secs(now_secs().saturating_sub(self.last_used.load(Ordering::Relaxed)))
    }

    pub fn is_alive(&self) -> bool {
        self.peer.is_alive()
    }

    /// Ask politely, then insist. A server that ignores `shutdown` still has to
    /// go, or a long session accumulates rust-analyzers.
    pub async fn shutdown(&self) {
        let _ = self
            .peer
            .request("shutdown", Value::Null, Duration::from_secs(2))
            .await;
        let _ = self.peer.notify("exit", Value::Null);

        let mut child = self.child.lock().await;
        for _ in 0..20 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}

async fn handshake(peer: Peer, root: PathBuf) -> Result<ServerCaps, Arc<anyhow::Error>> {
    let result = initialize(&peer, &root).await.map_err(Arc::new)?;
    let caps = read_caps(&result);
    peer.notify("initialized", json!({})).map_err(Arc::new)?;
    // Some servers only apply defaults once told settings changed.
    let _ = peer.notify("workspace/didChangeConfiguration", json!({ "settings": {} }));
    debug!(?root, "lsp: server initialized");
    Ok(caps)
}

async fn initialize(peer: &Peer, root: &Path) -> Result<Value> {
    let uri = path_to_uri(root);
    peer.request(
        "initialize",
        json!({
            "processId": std::process::id(),
            "clientInfo": { "name": "opman", "version": env!("CARGO_PKG_VERSION") },
            "rootUri": uri,
            "workspaceFolders": [{
                "uri": uri,
                "name": root.file_name().map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".into()),
            }],
            "capabilities": {
                "general": { "positionEncodings": ["utf-8", "utf-16"] },
                "workspace": {
                    "workspaceFolders": true,
                    "configuration": true,
                    "didChangeConfiguration": { "dynamicRegistration": false },
                },
                "textDocument": {
                    "synchronization": {
                        "dynamicRegistration": false,
                        "willSave": false,
                        "willSaveWaitUntil": false,
                        "didSave": true,
                    },
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "completion": {
                        "dynamicRegistration": false,
                        "contextSupport": true,
                        "completionItem": {
                            "snippetSupport": true,
                            "documentationFormat": ["markdown", "plaintext"],
                            "insertReplaceSupport": true,
                            "labelDetailsSupport": true,
                            "resolveSupport": {
                                "properties": ["documentation", "detail"]
                            }
                        },
                        "completionItemKind": { "valueSet": COMPLETION_KINDS }
                    },
                    "definition": { "linkSupport": true },
                    "formatting": { "dynamicRegistration": false },
                    "publishDiagnostics": {
                        "relatedInformation": false,
                        "versionSupport": false,
                        "tagSupport": { "valueSet": [1, 2] },
                    },
                },
                "window": { "workDoneProgress": true },
            },
        }),
        INIT_TIMEOUT,
    )
    .await
}

fn read_caps(result: &Value) -> ServerCaps {
    let caps = result.get("capabilities");
    let claims = |name: &str| {
        caps.and_then(|c| c.get(name))
            .map(|v| !matches!(v, Value::Null | Value::Bool(false)))
            .unwrap_or(false)
    };
    let completion_provider = caps.and_then(|c| c.get("completionProvider"));
    let trigger_characters = completion_provider
        .and_then(|provider| provider.get("triggerCharacters"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    ServerCaps {
        hover: claims("hoverProvider"),
        definition: claims("definitionProvider"),
        formatting: claims("documentFormattingProvider"),
        completion: completion_provider.is_some(),
        trigger_characters,
        encoding: PositionEncoding::from_server(
            caps.and_then(|c| c.get("positionEncoding"))
                .and_then(Value::as_str),
        ),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
