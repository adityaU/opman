//! PTY manager thread: owns all web PTY instances and processes commands.

use std::collections::HashMap;
use std::io::Write;

use portable_pty::PtySize;
use tokio::sync::mpsc;

use super::commands::PtyCmd;
use super::handle::WebPtyHandle;
use super::kind::SpawnSpec;
use super::session::PtySession;
use super::spawn::{spawn_pty, WebPty};

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

/// A PTY per id, for the life of the server process.
type Ptys = HashMap<String, WebPty>;

async fn run_manager(mut cmd_rx: mpsc::UnboundedReceiver<PtyCmd>) {
    let mut ptys: Ptys = HashMap::new();

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            PtyCmd::Spawn { spec, reply } => {
                let _ = reply.send(start(&mut ptys, &spec));
            }
            PtyCmd::Write { id, data, reply } => {
                let ok = ptys.get_mut(&id).is_some_and(|pty| {
                    pty.writer.write_all(&data).is_ok() && pty.writer.flush().is_ok()
                });
                let _ = reply.send(ok);
            }
            PtyCmd::Resize {
                id,
                rows,
                cols,
                reply,
            } => {
                let _ = reply.send(resize(&mut ptys, &id, rows, cols));
            }
            PtyCmd::GetOutput { id, reply } => {
                let _ = reply.send(ptys.get(&id).map(|pty| pty.output.clone()));
            }
            PtyCmd::Activity { id, reply } => {
                let _ = reply.send(ptys.get(&id).map(WebPty::activity));
            }
            PtyCmd::Rename { id, label, reply } => {
                let found = ptys.get_mut(&id);
                let ok = found.is_some();
                if let Some(pty) = found {
                    pty.meta.label = label;
                }
                let _ = reply.send(ok);
            }
            PtyCmd::Kill { id, reply } => {
                let killed = ptys.remove(&id);
                let ok = killed.is_some();
                if let Some(mut pty) = killed {
                    let _ = pty.child.kill();
                }
                let _ = reply.send(ok);
            }
            PtyCmd::Sessions { reply } => {
                let _ = reply.send(sessions(&mut ptys));
            }
        }
    }

    // Clean up all PTYs on shutdown
    for (_, mut pty) in ptys.drain() {
        let _ = pty.child.kill();
    }
}

/// Start a PTY, unless one already answers to that id.
///
/// Re-spawning over a live id would leave the running program with no reader
/// and no way to be killed, so an id that is already taken returns the PTY it
/// names. That makes spawn safe to retry, which is what a browser that
/// re-attaches after a reload effectively does.
fn start(ptys: &mut Ptys, spec: &SpawnSpec) -> Result<super::buffer::RawOutputBuffer, String> {
    if let Some(existing) = ptys.get(&spec.id) {
        return Ok(existing.output.clone());
    }

    let label = match &spec.label {
        Some(given) => given.clone(),
        None => next_label(ptys, spec),
    };

    let pty = spawn_pty(spec, label).map_err(|e| e.to_string())?;
    let output = pty.output.clone();
    ptys.insert(spec.id.clone(), pty);
    Ok(output)
}

/// "Shell 3" — numbered within its project and kind, so a second project's
/// first shell is Shell 1 rather than continuing someone else's count.
fn next_label(ptys: &Ptys, spec: &SpawnSpec) -> String {
    let kind = spec.program.kind();
    let taken = ptys
        .values()
        .filter(|pty| pty.meta.kind == kind && pty.meta.project == spec.project)
        .count();
    format!("{} {}", kind.label(), taken + 1)
}

fn resize(ptys: &mut Ptys, id: &str, rows: u16, cols: u16) -> bool {
    let Some(pty) = ptys.get_mut(id) else {
        return false;
    };
    let resized = pty
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .is_ok();
    if resized {
        pty.rows = rows;
        pty.cols = cols;
    }
    resized
}

/// Describe every live PTY, dropping the ones whose program has exited.
///
/// Pruning here rather than on a timer: the answer has to be accurate at the
/// moment it is asked, and there is no other moment worth spending a wakeup on.
fn sessions(ptys: &mut Ptys) -> Vec<PtySession> {
    ptys.retain(|_, pty| pty.is_alive());
    ptys.iter()
        .map(|(id, pty)| PtySession::new(id, &pty.meta, pty.activity()))
        .collect()
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod manager_tests;

#[cfg(test)]
#[path = "pty_test_support.rs"]
pub(crate) mod pty_test_support;

#[cfg(test)]
#[path = "manager_real_tests.rs"]
mod manager_real_tests;
