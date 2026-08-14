//! Async-safe handle to the web PTY manager (Clone + Send for Axum state).

use tokio::sync::{mpsc, oneshot};

use super::activity::PtyActivity;
use super::buffer::RawOutputBuffer;
use super::commands::PtyCmd;
use super::kind::SpawnSpec;
use super::session::PtySession;

/// Async-safe handle to the web PTY manager. Cloneable for Axum state.
#[derive(Clone)]
pub struct WebPtyHandle {
    pub(crate) cmd_tx: mpsc::UnboundedSender<PtyCmd>,
}

/// Ask the manager a question, or give up if it is not running.
///
/// Every method below is the same three lines around a different command, and
/// writing them out five times is how one of them ends up swallowing an error
/// the others report.
macro_rules! ask {
    ($self:expr, $build:expr, $fallback:expr) => {{
        let (tx, rx) = oneshot::channel();
        if $self.cmd_tx.send($build(tx)).is_err() {
            return $fallback;
        }
        rx.await.unwrap_or_else(|_| $fallback)
    }};
}

impl WebPtyHandle {
    /// Start a PTY and return its raw output buffer for SSE streaming.
    ///
    /// An id that is already live returns that PTY's buffer rather than
    /// starting a second program on top of it.
    pub async fn spawn(&self, spec: SpawnSpec) -> Result<RawOutputBuffer, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(PtyCmd::Spawn {
                spec: Box::new(spec),
                reply: tx,
            })
            .map_err(|_| "PTY manager not running".to_string())?;
        rx.await.map_err(|_| "PTY manager dropped".to_string())?
    }

    /// Write bytes to a web PTY.
    pub async fn write(&self, id: &str, data: Vec<u8>) -> bool {
        let id = id.to_owned();
        ask!(
            self,
            |reply| PtyCmd::Write { id, data, reply },
            false
        )
    }

    /// Resize a web PTY.
    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> bool {
        let id = id.to_owned();
        ask!(
            self,
            |reply| PtyCmd::Resize {
                id,
                rows,
                cols,
                reply
            },
            false
        )
    }

    /// Get the raw output buffer for a PTY (for SSE streaming).
    pub async fn get_output(&self, id: &str) -> Option<RawOutputBuffer> {
        let id = id.to_owned();
        ask!(self, |reply| PtyCmd::GetOutput { id, reply }, None)
    }

    /// Whether a PTY is running a foreground command. `None` when it is gone.
    pub async fn activity(&self, id: &str) -> Option<PtyActivity> {
        let id = id.to_owned();
        ask!(self, |reply| PtyCmd::Activity { id, reply }, None)
    }

    /// Rename a PTY as the shell picker shows it.
    pub async fn rename(&self, id: &str, label: String) -> bool {
        let id = id.to_owned();
        ask!(self, |reply| PtyCmd::Rename { id, label, reply }, false)
    }

    /// Kill a web PTY.
    pub async fn kill(&self, id: &str) -> bool {
        let id = id.to_owned();
        ask!(self, |reply| PtyCmd::Kill { id, reply }, false)
    }

    /// Every live PTY, with the project it belongs to and what it is doing.
    pub async fn sessions(&self) -> Vec<PtySession> {
        ask!(self, |reply| PtyCmd::Sessions { reply }, Vec::new())
    }

    /// Just the ids of the live PTYs, for callers that need nothing else.
    pub async fn list(&self) -> Vec<String> {
        self.sessions()
            .await
            .into_iter()
            .map(|session| session.id)
            .collect()
    }
}

#[cfg(test)]
#[path = "handle_tests.rs"]
mod handle_tests;
