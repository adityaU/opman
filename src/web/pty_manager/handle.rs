//! Async-safe handle to the web PTY manager (Clone + Send for Axum state).

use tokio::sync::{mpsc, oneshot};

use super::buffer::RawOutputBuffer;
use super::commands::PtyCmd;

/// Async-safe handle to the web PTY manager. Cloneable for Axum state.
#[derive(Clone)]
pub struct WebPtyHandle {
    pub(crate) cmd_tx: mpsc::UnboundedSender<PtyCmd>,
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
