//! PTY manager thread: owns all web PTY instances and processes commands.

use std::collections::HashMap;
use std::io::Write;

use portable_pty::PtySize;
use tokio::sync::mpsc;

use super::commands::PtyCmd;
use super::handle::WebPtyHandle;
use super::spawn::{
    spawn_gitui_pty, spawn_neovim_pty, spawn_opencode_pty, spawn_shell_pty, WebPty,
};

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
