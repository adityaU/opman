//! Web-specific PTY manager.
//!
//! Owns independent PTY instances for the web UI — completely separate from
//! the TUI's PTYs. Each web terminal panel (shell, neovim, gitui, opencode)
//! gets its own process.
//!
//! Raw PTY output bytes are captured into per-PTY ring buffers so that
//! xterm.js receives genuine VT100 escape sequences (not stripped text).
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

// ── Raw output buffer ───────────────────────────────────────────────

/// Thread-safe buffer that accumulates raw PTY output bytes.
/// The SSE stream reads from `read_pos` and the reader thread appends.
#[derive(Clone)]
pub struct RawOutputBuffer {
    inner: Arc<Mutex<RawOutputInner>>,
    pub dirty: Arc<AtomicBool>,
}

struct RawOutputInner {
    buf: Vec<u8>,
    /// How many bytes have been consumed by the SSE reader.
    read_pos: usize,
}

impl RawOutputBuffer {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RawOutputInner {
                buf: Vec::with_capacity(64 * 1024),
                read_pos: 0,
            })),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Append raw bytes (called from PTY reader thread).
    fn push(&self, data: &[u8]) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.buf.extend_from_slice(data);
            // Compact if buffer grows too large and read_pos is past halfway
            let rp = inner.read_pos;
            if inner.buf.len() > 1_000_000 && rp > inner.buf.len() / 2 {
                inner.buf.drain(..rp);
                inner.read_pos = 0;
            }
            self.dirty.store(true, Ordering::Release);
        }
    }

    /// Read any new bytes since last call. Returns empty slice if nothing new.
    pub fn drain_new(&self) -> Vec<u8> {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.read_pos < inner.buf.len() {
                let data = inner.buf[inner.read_pos..].to_vec();
                inner.read_pos = inner.buf.len();
                self.dirty.store(false, Ordering::Release);
                return data;
            }
        }
        Vec::new()
    }
}

// ── Web PTY instance ────────────────────────────────────────────────

/// A PTY owned by the web UI (not shared with TUI).
struct WebPty {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    output: RawOutputBuffer,
    rows: u16,
    cols: u16,
}

// ── Commands sent to the PTY manager thread ─────────────────────────

pub enum PtyCmd {
    /// Spawn a shell PTY for the web UI.
    SpawnShell {
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
        reply: oneshot::Sender<Result<RawOutputBuffer, String>>,
    },
    /// Spawn a neovim PTY for the web UI.
    SpawnNeovim {
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
        reply: oneshot::Sender<Result<RawOutputBuffer, String>>,
    },
    /// Spawn a gitui PTY for the web UI.
    SpawnGitui {
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
        reply: oneshot::Sender<Result<RawOutputBuffer, String>>,
    },
    /// Spawn an opencode attach PTY for the web UI.
    SpawnOpencode {
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
        /// If set, attach to this specific session; otherwise create a new one.
        session_id: Option<String>,
        reply: oneshot::Sender<Result<RawOutputBuffer, String>>,
    },
    /// Write bytes to a web PTY.
    Write {
        id: String,
        data: Vec<u8>,
        reply: oneshot::Sender<bool>,
    },
    /// Resize a web PTY.
    Resize {
        id: String,
        rows: u16,
        cols: u16,
        reply: oneshot::Sender<bool>,
    },
    /// Get the output buffer handle for SSE streaming.
    GetOutput {
        id: String,
        reply: oneshot::Sender<Option<RawOutputBuffer>>,
    },
    /// Kill and remove a web PTY.
    Kill {
        id: String,
        reply: oneshot::Sender<bool>,
    },
    /// List active web PTY IDs.
    List { reply: oneshot::Sender<Vec<String>> },
}

// ── Public handle (Clone, Send, used by Axum handlers) ──────────────

/// Async-safe handle to the web PTY manager. Cloneable for Axum state.
#[derive(Clone)]
pub struct WebPtyHandle {
    cmd_tx: mpsc::UnboundedSender<PtyCmd>,
}

impl WebPtyHandle {
    /// Spawn a shell and return its raw output buffer for SSE streaming.
    pub async fn spawn_shell(
        &self,
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
    ) -> Result<RawOutputBuffer, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::SpawnShell {
                id,
                rows,
                cols,
                working_dir,
                reply: tx,
            })
            .map_err(|_| "PTY manager not running".to_string())?;
        rx.await.map_err(|_| "PTY manager dropped".to_string())?
    }

    /// Spawn neovim and return its raw output buffer.
    pub async fn spawn_neovim(
        &self,
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
    ) -> Result<RawOutputBuffer, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::SpawnNeovim {
                id,
                rows,
                cols,
                working_dir,
                reply: tx,
            })
            .map_err(|_| "PTY manager not running".to_string())?;
        rx.await.map_err(|_| "PTY manager dropped".to_string())?
    }

    /// Spawn gitui and return its raw output buffer.
    pub async fn spawn_gitui(
        &self,
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
    ) -> Result<RawOutputBuffer, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::SpawnGitui {
                id,
                rows,
                cols,
                working_dir,
                reply: tx,
            })
            .map_err(|_| "PTY manager not running".to_string())?;
        rx.await.map_err(|_| "PTY manager dropped".to_string())?
    }

    /// Spawn `opencode attach` and return its raw output buffer.
    pub async fn spawn_opencode(
        &self,
        id: String,
        rows: u16,
        cols: u16,
        working_dir: std::path::PathBuf,
        session_id: Option<String>,
    ) -> Result<RawOutputBuffer, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::SpawnOpencode {
                id,
                rows,
                cols,
                working_dir,
                session_id,
                reply: tx,
            })
            .map_err(|_| "PTY manager not running".to_string())?;
        rx.await.map_err(|_| "PTY manager dropped".to_string())?
    }

    /// Write bytes to a web PTY.
    pub async fn write(&self, id: &str, data: Vec<u8>) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(PtyCmd::Write {
                id: id.to_string(),
                data,
                reply: tx,
            })
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    /// Resize a web PTY.
    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(PtyCmd::Resize {
                id: id.to_string(),
                rows,
                cols,
                reply: tx,
            })
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    /// Get the raw output buffer for a PTY (for SSE streaming).
    pub async fn get_output(&self, id: &str) -> Option<RawOutputBuffer> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::GetOutput {
                id: id.to_string(),
                reply: tx,
            })
            .ok()?;
        rx.await.ok()?
    }

    /// Kill a web PTY.
    pub async fn kill(&self, id: &str) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(PtyCmd::Kill {
                id: id.to_string(),
                reply: tx,
            })
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    /// List active web PTY IDs.
    pub async fn list(&self) -> Vec<String> {
        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send(PtyCmd::List { reply: tx }).is_err() {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

// ── Manager thread ──────────────────────────────────────────────────

/// Start the web PTY manager on a dedicated OS thread.
/// Returns a `WebPtyHandle` that can be cloned into Axum state.
pub fn start_web_pty_manager() -> WebPtyHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<PtyCmd>();

    std::thread::Builder::new()
        .name("web-pty-manager".into())
        .spawn(move || {
            // Create a single-threaded tokio runtime for the channel receiver
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create web PTY manager runtime");

            rt.block_on(async move {
                run_manager(cmd_rx).await;
            });
        })
        .expect("Failed to spawn web PTY manager thread");

    WebPtyHandle { cmd_tx }
}

async fn run_manager(mut cmd_rx: mpsc::UnboundedReceiver<PtyCmd>) {
    let mut ptys: HashMap<String, WebPty> = HashMap::new();

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            PtyCmd::SpawnShell {
                id,
                rows,
                cols,
                working_dir,
                reply,
            } => {
                let result = spawn_shell_pty(rows, cols, &working_dir);
                match result {
                    Ok(pty) => {
                        let output = pty.output.clone();
                        ptys.insert(id, pty);
                        let _ = reply.send(Ok(output));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e.to_string()));
                    }
                }
            }
            PtyCmd::SpawnNeovim {
                id,
                rows,
                cols,
                working_dir,
                reply,
            } => {
                let result = spawn_neovim_pty(rows, cols, &working_dir);
                match result {
                    Ok(pty) => {
                        let output = pty.output.clone();
                        ptys.insert(id, pty);
                        let _ = reply.send(Ok(output));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e.to_string()));
                    }
                }
            }
            PtyCmd::SpawnGitui {
                id,
                rows,
                cols,
                working_dir,
                reply,
            } => {
                let result = spawn_gitui_pty(rows, cols, &working_dir);
                match result {
                    Ok(pty) => {
                        let output = pty.output.clone();
                        ptys.insert(id, pty);
                        let _ = reply.send(Ok(output));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e.to_string()));
                    }
                }
            }
            PtyCmd::SpawnOpencode {
                id,
                rows,
                cols,
                working_dir,
                session_id,
                reply,
            } => {
                let result = spawn_opencode_pty(rows, cols, &working_dir, session_id.as_deref());
                match result {
                    Ok(pty) => {
                        let output = pty.output.clone();
                        ptys.insert(id, pty);
                        let _ = reply.send(Ok(output));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e.to_string()));
                    }
                }
            }
            PtyCmd::Write { id, data, reply } => {
                let ok = if let Some(pty) = ptys.get_mut(&id) {
                    pty.writer.write_all(&data).is_ok() && pty.writer.flush().is_ok()
                } else {
                    false
                };
                let _ = reply.send(ok);
            }
            PtyCmd::Resize {
                id,
                rows,
                cols,
                reply,
            } => {
                let ok = if let Some(pty) = ptys.get_mut(&id) {
                    let resize_ok = pty
                        .master
                        .resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .is_ok();
                    if resize_ok {
                        pty.rows = rows;
                        pty.cols = cols;
                    }
                    resize_ok
                } else {
                    false
                };
                let _ = reply.send(ok);
            }
            PtyCmd::GetOutput { id, reply } => {
                let output = ptys.get(&id).map(|pty| pty.output.clone());
                let _ = reply.send(output);
            }
            PtyCmd::Kill { id, reply } => {
                let ok = if let Some(mut pty) = ptys.remove(&id) {
                    let _ = pty.child.kill();
                    true
                } else {
                    false
                };
                let _ = reply.send(ok);
            }
            PtyCmd::List { reply } => {
                let ids: Vec<String> = ptys.keys().cloned().collect();
                let _ = reply.send(ids);
            }
        }
    }

    // Clean up all PTYs on shutdown
    for (_, mut pty) in ptys.drain() {
        let _ = pty.child.kill();
    }
}

// ── PTY spawn helpers ───────────────────────────────────────────────

/// Background reader that captures raw bytes into the output buffer.
fn read_raw_pty_output(mut reader: Box<dyn Read + Send>, output: RawOutputBuffer) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                output.push(&buf[..n]);
            }
            Err(_) => break,
        }
    }
}

fn spawn_shell_pty(rows: u16, cols: u16, working_dir: &std::path::Path) -> Result<WebPty> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web shell")?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(%shell, rows, cols, ?working_dir, "Web shell PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

fn spawn_neovim_pty(rows: u16, cols: u16, working_dir: &std::path::Path) -> Result<WebPty> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web neovim")?;

    let mut cmd = CommandBuilder::new("nvim");
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Simple neovim with line numbers
    cmd.arg("--cmd");
    cmd.arg("set signcolumn=yes number norelativenumber noswapfile");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn neovim in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-nvim".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(rows, cols, ?working_dir, "Web neovim PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

fn spawn_gitui_pty(rows: u16, cols: u16, working_dir: &std::path::Path) -> Result<WebPty> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web gitui")?;

    let mut cmd = CommandBuilder::new("gitui");
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn gitui in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-git".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(rows, cols, ?working_dir, "Web gitui PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

fn spawn_opencode_pty(
    rows: u16,
    cols: u16,
    working_dir: &std::path::Path,
    session_id: Option<&str>,
) -> Result<WebPty> {
    let base_url = crate::app::base_url();
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web opencode")?;

    let mut cmd = CommandBuilder::new("opencode");
    cmd.arg("attach");
    cmd.arg(base_url);
    cmd.arg("--dir");
    cmd.arg(working_dir.to_string_lossy().as_ref());
    if let Some(sid) = session_id {
        cmd.arg("--session");
        cmd.arg(sid);
    }
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn opencode attach in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-opencode".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(
        rows,
        cols,
        ?working_dir,
        ?session_id,
        "Web opencode PTY spawned"
    );

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}
