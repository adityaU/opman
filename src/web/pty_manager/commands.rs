//! Command enum sent to the PTY manager thread.

use tokio::sync::oneshot;

use super::activity::PtyActivity;
use super::buffer::RawOutputBuffer;
use super::kind::SpawnSpec;
use super::session::PtySession;

pub enum PtyCmd {
    /// Start a PTY. One variant for every kind: what differs between them is
    /// the command line, which `SpawnSpec` already carries.
    Spawn {
        spec: Box<SpawnSpec>,
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
    /// Whether a PTY is running a command, or `None` if there is no such PTY.
    Activity {
        id: String,
        reply: oneshot::Sender<Option<PtyActivity>>,
    },
    /// Rename a PTY as it appears in the shell picker.
    Rename {
        id: String,
        label: String,
        reply: oneshot::Sender<bool>,
    },
    /// Kill and remove a web PTY.
    Kill {
        id: String,
        reply: oneshot::Sender<bool>,
    },
    /// Every live PTY with its project, label and activity. Exited programs are
    /// dropped as part of answering, so this is also what prunes the map.
    Sessions {
        reply: oneshot::Sender<Vec<PtySession>>,
    },
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod commands_tests;
