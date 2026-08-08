//! The plumbing behind a terminal: how a command's bytes are kept, and how it ended.
//!
//! Split from [`super::terminal`], which owns the protocol methods. Nothing here knows what
//! ACP is — it is a bounded buffer and an exit status — which is what keeps the method module
//! about the lifecycle the agent drives.

use std::process::ExitStatus;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, ChildStdout};

/// How a command ended. `Copy`, so a waiter can take it out of the watch channel without
/// holding the borrow across an await.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Exit {
    code: Option<i32>,
    signal: Option<i32>,
}

impl Exit {
    pub(super) fn of(status: ExitStatus) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            Self {
                code: status.code(),
                signal: status.signal(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                code: status.code(),
                signal: None,
            }
        }
    }

    /// ACP's `TerminalExitStatus`: a code or a signal *name*, both nullable.
    pub(super) fn to_value(self) -> Value {
        json!({
            "exitCode": self.code,
            "signal": self.signal.map(signal_name),
        })
    }
}

/// The signals a command actually dies of, named as a shell names them. Anything else is
/// reported by number, which is still more use to the agent than dropping it.
fn signal_name(signal: i32) -> String {
    match signal {
        1 => "SIGHUP".to_string(),
        2 => "SIGINT".to_string(),
        3 => "SIGQUIT".to_string(),
        6 => "SIGABRT".to_string(),
        9 => "SIGKILL".to_string(),
        13 => "SIGPIPE".to_string(),
        15 => "SIGTERM".to_string(),
        other => other.to_string(),
    }
}

/// The tail of a command's output, and whether keeping it to size cost anything.
pub(super) struct Buffer {
    bytes: Vec<u8>,
    truncated: bool,
    limit: usize,
}

impl Buffer {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
            limit,
        }
    }

    /// Append, dropping from the front to stay inside the limit.
    ///
    /// The protocol requires the retained output to remain a valid string, so the drop
    /// advances to the next UTF-8 boundary rather than cutting a character in half — which
    /// would corrupt the whole tail, not just the byte it landed on.
    pub(super) fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);
        if self.bytes.len() <= self.limit {
            return;
        }
        let excess = self.bytes.len() - self.limit;
        self.bytes.drain(..excess);
        let boundary = self
            .bytes
            .iter()
            .position(|byte| byte & 0b1100_0000 != 0b1000_0000)
            .unwrap_or(self.bytes.len());
        self.bytes.drain(..boundary);
        self.truncated = true;
    }

    pub(super) fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub(super) fn truncated(&self) -> bool {
        self.truncated
    }
}

/// One of the child's two output pipes. An enum rather than a boxed `AsyncRead`, so the read
/// loop stays a static dispatch over the only two things it will ever be handed.
pub(super) enum Pipe {
    Out(ChildStdout),
    Err(ChildStderr),
}

/// Copy a pipe into the shared buffer until it closes.
pub(super) fn drain(mut pipe: Pipe, buffer: &Arc<Mutex<Buffer>>) -> tokio::task::JoinHandle<()> {
    let buffer = buffer.clone();
    tokio::spawn(async move {
        let mut chunk = [0u8; 8192];
        loop {
            let read = match &mut pipe {
                Pipe::Out(out) => out.read(&mut chunk).await,
                Pipe::Err(err) => err.read(&mut chunk).await,
            };
            match read {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if let Ok(mut buffer) = buffer.lock() {
                        buffer.push(&chunk[..n]);
                    }
                }
            }
        }
    })
}

#[cfg(test)]
#[path = "terminal_io_tests.rs"]
mod terminal_io_tests;
