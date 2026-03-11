//! Command enum sent to the PTY manager thread.

use tokio::sync::oneshot;

use super::buffer::RawOutputBuffer;

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
