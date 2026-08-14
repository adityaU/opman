//! Opening a PTY pair and starting a program in it.

use std::io::{Read, Write};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tracing::debug;

use super::activity::PtyActivity;
use super::buffer::RawOutputBuffer;
use super::kind::{PtyProgram, SpawnSpec};
use super::session::PtyMeta;

/// A PTY owned by the web UI (not shared with TUI).
pub(crate) struct WebPty {
    pub(crate) writer: Box<dyn Write + Send>,
    pub(crate) master: Box<dyn MasterPty + Send>,
    pub(crate) child: Box<dyn portable_pty::Child + Send + Sync>,
    pub(crate) output: RawOutputBuffer,
    pub(crate) meta: PtyMeta,
    pub(crate) rows: u16,
    pub(crate) cols: u16,
}

impl WebPty {
    /// Whether a process other than the spawned program owns the terminal.
    ///
    /// Cheap enough to call on a timer: `process_group_leader` is one
    /// `tcgetpgrp` on the master fd, with no allocation and no child reaping.
    pub(crate) fn activity(&self) -> PtyActivity {
        PtyActivity::classify(self.foreground_group(), self.child.process_id())
    }

    /// Whether the program is still running.
    ///
    /// A shell the user typed `exit` into leaves a PTY behind that reads and
    /// writes nothing. Without this the picker would keep offering it forever,
    /// since nothing else in the process notices a child ending.
    pub(crate) fn is_alive(&mut self) -> bool {
        // An error here means the child cannot be waited on at all, which is
        // not a state it can recover from — treat it as gone rather than
        // stranding the entry.
        matches!(self.child.try_wait(), Ok(None))
    }

    #[cfg(unix)]
    fn foreground_group(&self) -> Option<i32> {
        self.master.process_group_leader()
    }

    /// Nothing equivalent exists on Windows ConPTY, so every PTY reads idle.
    #[cfg(not(unix))]
    fn foreground_group(&self) -> Option<i32> {
        None
    }
}

/// Background reader that captures raw bytes into the output buffer.
fn read_raw_pty_output(mut reader: Box<dyn Read + Send>, output: RawOutputBuffer) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output.push(&buf[..n]),
            Err(_) => break,
        }
    }
}

/// The command one program runs, in the project it was asked for.
///
/// This is the only part of spawning that differs per kind. It used to be five
/// functions that were otherwise byte-for-byte identical, so a fix to the PTY
/// plumbing had to be made five times and in practice was made once.
fn command_for(program: &PtyProgram, project: &std::path::Path) -> CommandBuilder {
    let mut cmd = match program {
        PtyProgram::Shell => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            CommandBuilder::new(shell)
        }
        PtyProgram::Neovim => {
            let mut cmd = CommandBuilder::new("nvim");
            cmd.arg("--cmd");
            cmd.arg("set signcolumn=yes number norelativenumber noswapfile");
            cmd
        }
        PtyProgram::Git => CommandBuilder::new("gitui"),
        PtyProgram::Opencode { session_id } => {
            let mut cmd = CommandBuilder::new("opencode");
            cmd.arg("attach");
            cmd.arg(crate::app::base_url());
            cmd.arg("--dir");
            cmd.arg(project.to_string_lossy().as_ref());
            if let Some(sid) = session_id {
                cmd.arg("--session");
                cmd.arg(sid);
            }
            cmd
        }
        PtyProgram::ClaudeAttach { short_id } => {
            let bin = std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
            let mut cmd = CommandBuilder::new(bin);
            cmd.arg("attach");
            cmd.arg(short_id);
            cmd
        }
    };

    cmd.cwd(project);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd
}

/// Open a PTY pair, start the program in it, and stream its bytes into a buffer.
pub(crate) fn spawn_pty(spec: &SpawnSpec, label: String) -> Result<WebPty> {
    let kind = spec.program.kind();
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: spec.rows,
            cols: spec.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .with_context(|| format!("Failed to open PTY pair for {}", kind.label()))?;

    let child = pair
        .slave
        .spawn_command(command_for(&spec.program, &spec.project))
        .with_context(|| format!("Failed to spawn {} in web PTY", kind.label()))?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;
    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let sink = output.clone();
    std::thread::Builder::new()
        .name(format!("web-pty-reader-{}", kind.label().to_lowercase()))
        .spawn(move || read_raw_pty_output(reader, sink))
        .context("Failed to spawn web PTY reader thread")?;

    debug!(
        ?kind,
        rows = spec.rows,
        cols = spec.cols,
        project = ?spec.project,
        %label,
        "Web PTY spawned"
    );

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        meta: PtyMeta {
            kind,
            label,
            project: spec.project.clone(),
        },
        rows: spec.rows,
        cols: spec.cols,
    })
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod spawn_tests;

#[cfg(test)]
#[path = "spawn_real_tests.rs"]
mod spawn_real_tests;
