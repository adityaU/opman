//! Saying exactly why an agent would not start.
//!
//! An agent that dies during `initialize` usually explains itself on stderr, and a JSON-RPC
//! refusal carries the reason in its message and data. opman used to throw the first away
//! (stderr was null) and show only the outer layer of the second ("ACP `initialize` failed").
//! This module keeps the tail of stderr, and joins it, the exit status and the full error
//! chain into one text the user can read in the session.

use std::process::ExitStatus;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::anyhow;

use super::AcpEngine;
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::sync::watch;

/// How much of the agent's stderr is kept. Only the end matters: that is where it says why.
const KEEP_BYTES: usize = 2048;
/// How long to wait for stderr to run dry once the process is known to be gone.
const DRAIN_WAIT: Duration = Duration::from_millis(300);

/// The last bytes an agent wrote to stderr, filled by a background reader.
#[derive(Clone)]
pub(super) struct StderrTail {
    bytes: Arc<Mutex<Vec<u8>>>,
    /// Turns true when the pipe reaches EOF.
    drained: watch::Receiver<bool>,
}

impl StderrTail {
    /// Start reading the child's stderr. A child without a pipe gives an empty, finished tail.
    pub(super) fn capture(child: &mut Child) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (done, drained) = watch::channel(false);
        let tail = Self {
            bytes: bytes.clone(),
            drained,
        };
        let Some(mut pipe) = child.stderr.take() else {
            done.send_replace(true);
            return tail;
        };
        tokio::spawn(async move {
            let mut chunk = [0u8; 1024];
            while let Ok(read) = pipe.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                push_tail(&bytes, &chunk[..read]);
            }
            done.send_replace(true);
        });
        tail
    }

    /// The kept text, after giving a closing pipe a moment to finish.
    pub(super) async fn text(&self) -> String {
        let mut drained = self.drained.clone();
        let _ = tokio::time::timeout(DRAIN_WAIT, drained.wait_for(|done| *done)).await;
        let Ok(bytes) = self.bytes.lock() else {
            return String::new();
        };
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}

fn push_tail(bytes: &Mutex<Vec<u8>>, chunk: &[u8]) {
    let Ok(mut kept) = bytes.lock() else {
        return;
    };
    kept.extend_from_slice(chunk);
    let extra = kept.len().saturating_sub(KEEP_BYTES);
    kept.drain(..extra);
}

impl AcpEngine {
    /// Remember why the agent could not start, for the settings row.
    pub(super) fn note_failure(&self, reason: String) {
        if let Ok(mut last) = self.last_failure.lock() {
            *last = Some(reason);
        }
    }

    /// The agent started: forget any earlier failure.
    pub(super) fn note_started(&self) {
        if let Ok(mut last) = self.last_failure.lock() {
            *last = None;
        }
    }

    /// Why the agent last failed to start, if it did.
    pub fn last_failure(&self) -> Option<String> {
        self.last_failure.lock().ok().and_then(|last| last.clone())
    }
}

/// Join an error chain, the exit status and the stderr tail into one message.
pub(super) async fn explain(
    error: anyhow::Error,
    tail: &StderrTail,
    exit: Option<ExitStatus>,
) -> anyhow::Error {
    anyhow!(compose(&format!("{error:#}"), exit, &tail.text().await))
}

fn compose(reason: &str, exit: Option<ExitStatus>, stderr: &str) -> String {
    let mut text = reason.to_string();
    if let Some(status) = exit {
        text.push_str(&format!("\nThe agent exited: {status}"));
    }
    if !stderr.is_empty() {
        text.push_str(&format!("\nAgent stderr (last part):\n{stderr}"));
    }
    text
}

/// A start that failed: note whether the process had already exited, stop it, and explain.
pub(super) async fn startup_failure(
    child: &mut Child,
    tail: &StderrTail,
    error: anyhow::Error,
) -> anyhow::Error {
    let exit = tokio::time::timeout(DRAIN_WAIT, child.wait())
        .await
        .ok()
        .and_then(Result::ok);
    let _ = child.start_kill();
    let _ = child.wait().await;
    explain(error, tail, exit).await
}

#[cfg(test)]
#[path = "failure_tests.rs"]
mod failure_tests;
