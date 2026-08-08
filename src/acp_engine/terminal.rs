//! `terminal/*`: running a command on the agent's behalf.
//!
//! The other half of `client_caps`. An agent that takes this offer stops shelling out inside
//! its own process and asks opman to do it, which is what puts the command under opman's
//! working directory, opman's environment, and opman's permission prompt — and what lets a
//! long-running command stream while the turn continues, instead of blocking it.
//!
//! The lifecycle is the agent's to drive: it creates a terminal, polls `output` or waits on
//! `wait_for_exit`, and releases when done. opman only keeps the process and its tail.
//!
//! Cancellation is a signal, not a lock. The child is owned by one task that both waits on it
//! and listens for a kill, so `terminal/kill` never has to take a handle away from the waiter
//! — the deadlock a shared `Mutex<Child>` would produce the moment an agent killed something
//! it was already waiting on.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::{watch, Notify};

use super::terminal_io::{drain, Buffer, Exit, Pipe};
use super::AcpEngine;

/// Output retained when the agent names no limit of its own.
const DEFAULT_BYTE_LIMIT: usize = 1 << 20;

/// One running (or finished) command.
struct Terminal {
    /// The opman session that asked for it, so ending a session takes its terminals with it.
    session: String,
    buffer: Arc<Mutex<Buffer>>,
    exit: watch::Receiver<Option<Exit>>,
    /// Signalled to kill the child. `notify_one` rather than `notify_waiters` because the
    /// permit has to survive being sent before the owning task reaches its `select`.
    kill: Arc<Notify>,
}

impl Terminal {
    fn output(&self) -> Value {
        let (text, truncated) = self
            .buffer
            .lock()
            .map(|buffer| (buffer.text(), buffer.truncated()))
            .unwrap_or_default();
        json!({
            "output": text,
            "truncated": truncated,
            "exitStatus": self.exit.borrow().map(Exit::to_value),
        })
    }
}

/// Every terminal opman is holding for this agent.
#[derive(Default)]
pub(super) struct Registry(Mutex<HashMap<String, Arc<Terminal>>>);

impl Registry {
    fn get(&self, id: &str) -> Option<Arc<Terminal>> {
        self.0.lock().ok()?.get(id).cloned()
    }

    fn insert(&self, id: String, terminal: Arc<Terminal>) {
        if let Ok(mut all) = self.0.lock() {
            all.insert(id, terminal);
        }
    }

    fn remove(&self, id: &str) -> Option<Arc<Terminal>> {
        self.0.lock().ok()?.remove(id)
    }

    /// What a terminal has printed so far, for a tool call that points at one instead of
    /// carrying its own content.
    pub(super) fn text(&self, id: &str) -> Option<String> {
        let terminal = self.get(id)?;
        let text = terminal.buffer.lock().ok()?.text();
        Some(text)
    }

    /// Kill and forget every terminal a session owns. A released session's commands have
    /// nothing left to report to, so leaving them running would be a leak with no reader.
    pub(super) fn release_session(&self, session: &str) {
        let Ok(mut all) = self.0.lock() else {
            return;
        };
        all.retain(|_, terminal| {
            let mine = terminal.session == session;
            if mine {
                terminal.kill.notify_one();
            }
            !mine
        });
    }
}

/// `terminal/create`: spawn a command and hand back a handle to it.
pub(super) async fn create(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    let session_id = permitted(engine, params, "terminal/create")?;
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
        .context("`terminal/create` requires a command")?;

    let mut child = build(engine, params, command, &session_id)?
        .spawn()
        .with_context(|| format!("failed to spawn `{command}`"))?;
    let limit = params
        .get("outputByteLimit")
        .and_then(Value::as_u64)
        .map(|limit| limit as usize)
        .unwrap_or(DEFAULT_BYTE_LIMIT);
    let buffer = Arc::new(Mutex::new(Buffer::new(limit)));

    // Both pipes feed one buffer, in the order the bytes arrive, which is what makes the
    // captured output read like the terminal it stands in for.
    let out = child
        .stdout
        .take()
        .map(|pipe| drain(Pipe::Out(pipe), &buffer));
    let err = child
        .stderr
        .take()
        .map(|pipe| drain(Pipe::Err(pipe), &buffer));

    let kill = Arc::new(Notify::new());
    let (exit_tx, exit_rx) = watch::channel(None);
    let signal = kill.clone();
    tokio::spawn(async move {
        let status = tokio::select! {
            // `Child::wait` is cancel safe, so losing this branch to a kill loses nothing.
            status = child.wait() => status,
            _ = signal.notified() => {
                let _ = child.start_kill();
                child.wait().await
            }
        };
        // Drain what the pipes still hold before reporting an exit: an agent that reads
        // output after `wait_for_exit` must see everything the command wrote.
        for reader in [out, err].into_iter().flatten() {
            let _ = reader.await;
        }
        // A child opman cannot reap ended in a way it has nothing to say about; reporting
        // "no code, no signal" still ends the wait, where staying silent would hang it.
        let _ = exit_tx.send(Some(status.map(Exit::of).unwrap_or_default()));
    });

    let id = super::rand_id("term");
    engine.terminals.insert(
        id.clone(),
        Arc::new(Terminal {
            session: session_id,
            buffer,
            exit: exit_rx,
            kill,
        }),
    );
    Ok(json!({ "terminalId": id }))
}

/// `terminal/output`: the tail so far, plus an exit status once there is one.
pub(super) fn output(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    Ok(locate(engine, params, "terminal/output")?.output())
}

/// `terminal/wait_for_exit`: block until the command ends.
pub(super) async fn wait_for_exit(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    let terminal = locate(engine, params, "terminal/wait_for_exit")?;
    // Take a receiver of our own: `changed` needs `&mut`, and a shared cursor would let one
    // waiter consume the notification another is still waiting for.
    let mut exit = terminal.exit.clone();
    loop {
        if let Some(status) = *exit.borrow_and_update() {
            return Ok(status.to_value());
        }
        // The sender is dropped only when the owning task is gone, which cannot happen
        // before it has sent — so this is a closed channel, not a command still running.
        if exit.changed().await.is_err() {
            bail!("the terminal ended without reporting an exit status");
        }
    }
}

/// `terminal/kill`: stop the command but keep its output readable.
pub(super) fn kill(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    locate(engine, params, "terminal/kill")?.kill.notify_one();
    Ok(json!({}))
}

/// `terminal/release`: kill if still running, and forget it. The agent is telling opman it
/// will not read this one again, so holding the output past here is holding it for nobody.
pub(super) fn release(engine: &Arc<AcpEngine>, params: &Value) -> Result<Value> {
    let id = terminal_id(params)?;
    if let Some(terminal) = engine.terminals.remove(id) {
        terminal.kill.notify_one();
    }
    Ok(json!({}))
}

/// The command to spawn: the agent's, under the session's directory unless it named another.
fn build(
    engine: &Arc<AcpEngine>,
    params: &Value,
    command: &str,
    session_id: &str,
) -> Result<Command> {
    let mut cmd = Command::new(command);
    if let Some(args) = params.get("args").and_then(Value::as_array) {
        cmd.args(args.iter().filter_map(Value::as_str));
    }
    for entry in params
        .get("env")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(name), Some(value)) = (
            entry.get("name").and_then(Value::as_str),
            entry.get("value").and_then(Value::as_str),
        ) {
            cmd.env(name, value);
        }
    }
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| engine.get_session(session_id).map(|s| s.directory))
        .filter(|dir| !dir.is_empty())
        .context("no working directory for the terminal")?;
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The agent may never release it, and a command outliving opman answers to nobody.
        .kill_on_drop(true);
    Ok(cmd)
}

/// The session behind an inbound terminal request, once the capability allows it at all.
fn permitted(engine: &Arc<AcpEngine>, params: &Value, method: &str) -> Result<String> {
    if !engine.agent.client_caps.terminal {
        bail!("opman does not implement `{method}`");
    }
    engine
        .opman_session(params)
        .with_context(|| format!("`{method}` for an unknown session"))
}

fn terminal_id(params: &Value) -> Result<&str> {
    params
        .get("terminalId")
        .and_then(Value::as_str)
        .context("`terminalId` is required")
}

fn locate(engine: &Arc<AcpEngine>, params: &Value, method: &str) -> Result<Arc<Terminal>> {
    permitted(engine, params, method)?;
    let id = terminal_id(params)?;
    engine
        .terminals
        .get(id)
        .with_context(|| format!("no terminal `{id}` — it may already have been released"))
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod terminal_tests;
