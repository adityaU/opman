//! PTY spawn helpers for shell, neovim, gitui, and opencode.

use std::io::{Read, Write};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tracing::debug;

use super::buffer::RawOutputBuffer;

/// A PTY owned by the web UI (not shared with TUI).
pub(crate) struct WebPty {
    pub(crate) writer: Box<dyn Write + Send>,
    pub(crate) master: Box<dyn MasterPty + Send>,
    pub(crate) child: Box<dyn portable_pty::Child + Send + Sync>,
    pub(crate) output: RawOutputBuffer,
    pub(crate) rows: u16,
    pub(crate) cols: u16,
}

/// Background reader that captures raw bytes into the output buffer.
fn read_raw_pty_output(mut reader: Box<dyn Read + Send>, output: RawOutputBuffer) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                output.push(&buf[..n]);
            }
            Err(_) => break,
        }
    }
}

pub(crate) fn spawn_shell_pty(
    rows: u16,
    cols: u16,
    working_dir: &std::path::Path,
) -> Result<WebPty> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web shell")?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(%shell, rows, cols, ?working_dir, "Web shell PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

pub(crate) fn spawn_neovim_pty(
    rows: u16,
    cols: u16,
    working_dir: &std::path::Path,
) -> Result<WebPty> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web neovim")?;

    let mut cmd = CommandBuilder::new("nvim");
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Simple neovim with line numbers
    cmd.arg("--cmd");
    cmd.arg("set signcolumn=yes number norelativenumber noswapfile");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn neovim in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-nvim".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(rows, cols, ?working_dir, "Web neovim PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

pub(crate) fn spawn_gitui_pty(
    rows: u16,
    cols: u16,
    working_dir: &std::path::Path,
) -> Result<WebPty> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web gitui")?;

    let mut cmd = CommandBuilder::new("gitui");
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn gitui in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-git".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(rows, cols, ?working_dir, "Web gitui PTY spawned");

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}

pub(crate) fn spawn_opencode_pty(
    rows: u16,
    cols: u16,
    working_dir: &std::path::Path,
    session_id: Option<&str>,
) -> Result<WebPty> {
    let base_url = crate::app::base_url();
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY pair for web opencode")?;

    let mut cmd = CommandBuilder::new("opencode");
    cmd.arg("attach");
    cmd.arg(base_url);
    cmd.arg("--dir");
    cmd.arg(working_dir.to_string_lossy().as_ref());
    if let Some(sid) = session_id {
        cmd.arg("--session");
        cmd.arg(sid);
    }
    cmd.cwd(working_dir);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn opencode attach in web PTY")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone web PTY reader")?;

    let writer = pair
        .master
        .take_writer()
        .context("Failed to get web PTY writer")?;

    let output = RawOutputBuffer::new();
    let output_clone = output.clone();

    std::thread::Builder::new()
        .name("web-pty-reader-opencode".into())
        .spawn(move || {
            read_raw_pty_output(reader, output_clone);
        })
        .context("Failed to spawn web PTY reader thread")?;

    debug!(
        rows,
        cols,
        ?working_dir,
        ?session_id,
        "Web opencode PTY spawned"
    );

    Ok(WebPty {
        writer,
        master: pair.master,
        child,
        output,
        rows,
        cols,
    })
}
