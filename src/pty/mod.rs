mod reader;
mod spawn_gitui;
mod spawn_neovim;
mod spawn_opencode;
mod spawn_shell;

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{Child, MasterPty, PtySize};

/// Tracks the state of the most recent command in a shell PTY.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandState {
    /// No command running (at prompt).
    Idle,
    /// A command is currently executing.
    Running,
    /// The last command completed successfully (exit code 0).
    Success,
    /// The last command failed (exit code != 0).
    Failure,
}

pub struct PtyInstance {
    pub parser: Arc<Mutex<vt100::Parser>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    pub rows: u16,
    pub cols: u16,
    master: Option<Box<dyn MasterPty + Send>>,
    pub scroll_offset: usize,
    pub name: String,
    pub command_state: Arc<Mutex<CommandState>>,
    /// Path to the neovim `--listen` socket (only set for neovim PTYs).
    pub nvim_listen_addr: Option<std::path::PathBuf>,
    /// Set to `true` by the reader thread when new PTY output arrives.
    /// Cleared by the render loop after drawing.  Used to skip
    /// expensive terminal re-renders when the screen hasn't changed.
    pub dirty: Arc<AtomicBool>,
    /// Epoch-millis timestamp of the last PTY output.  Updated by the reader
    /// thread whenever bytes arrive.  Read by hang detection to determine if
    /// a long-running tool call is still producing output.
    pub last_output_at: Arc<AtomicU64>,
}

impl std::fmt::Debug for PtyInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyInstance")
            .field("name", &self.name)
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("has_writer", &self.writer.is_some())
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl Drop for PtyInstance {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

impl PtyInstance {
    /// Returns `true` if the PTY has received new output since the last call.
    /// Atomically clears the flag.
    #[inline]
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// Write input bytes (e.g. keystrokes) to the PTY child process.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            // Detect Enter key → mark command as Running
            if data.contains(&b'\r') || data.contains(&b'\n') {
                if let Ok(mut state) = self.command_state.lock() {
                    *state = CommandState::Running;
                }
            }
            writer.write_all(data).context("Failed to write to PTY")?;
            writer.flush().context("Failed to flush PTY writer")?;
        }
        Ok(())
    }

    /// Resize the PTY (and inform the parser).
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if self.rows == rows && self.cols == cols {
            return Ok(());
        }
        self.rows = rows;
        self.cols = cols;

        if let Some(ref master) = self.master {
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .context("Failed to resize PTY")?;
        }

        if let Ok(mut parser) = self.parser.lock() {
            parser.set_size(rows, cols);
        }

        Ok(())
    }

    /// Check if the child process is still running.
    #[allow(dead_code)]
    pub fn is_alive(&mut self) -> bool {
        match &mut self.child {
            Some(child) => child.try_wait().ok().flatten().is_none(),
            None => false,
        }
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            child.kill().context("Failed to kill PTY child")?;
        }
        Ok(())
    }
}
