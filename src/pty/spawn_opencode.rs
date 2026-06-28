use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tracing::debug;

use super::reader;
use super::{CommandState, PtyInstance};

/// Build the engine command for a session's PTY pane.
///
/// - opencode backend: `opencode attach <url> --dir <dir> [--session <id>]`.
/// - claude backend (embedded engine present): `claude attach <shortid>` once the
///   session has a background agent; otherwise a placeholder that hints the user to
///   send a message (a brand-new Claude session has no attachable agent yet).
fn build_engine_command(
    url: &str,
    working_dir: &std::path::Path,
    session_id: Option<&str>,
) -> CommandBuilder {
    if crate::claude_engine::engine().is_some() {
        let short = session_id.and_then(crate::claude_engine::short_id_for_session);
        if let Some(short) = short {
            let mut c = CommandBuilder::new("claude");
            c.arg("attach");
            c.arg(short);
            if let Some(sid) = session_id {
                c.env("OPENCODE_SESSION_ID", sid);
            }
            return c;
        }
        let mut c = CommandBuilder::new("sh");
        c.arg("-c");
        c.arg("printf '\\n  Send a message to start this Claude session…\\n'; exec sleep 86400");
        return c;
    }

    let mut c = CommandBuilder::new("opencode");
    c.arg("attach");
    c.arg(url);
    c.arg("--dir");
    c.arg(working_dir);
    c.arg("--log-level");
    c.arg("ERROR");
    if let Some(sid) = session_id {
        c.arg("--session");
        c.arg(sid);
        // Export session ID as env var so MCP bridge processes (spawned by
        // opencode as children) can include it in socket requests, ensuring
        // terminal/neovim tool calls route to the correct session.
        c.env("OPENCODE_SESSION_ID", sid);
    }
    c
}

impl PtyInstance {
    /// Spawn a new PTY running `opencode attach <url>`.
    ///
    /// The PTY will capture the opencode TUI output via a VT100 parser.
    pub fn spawn(
        url: &str,
        rows: u16,
        cols: u16,
        working_dir: &std::path::Path,
        session_id: Option<&str>,
        theme_envs: &[(String, String)],
    ) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY pair")?;

        let mut cmd = build_engine_command(url, working_dir, session_id);
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, val) in theme_envs {
            cmd.env(key, val);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn opencode attach in PTY")?;

        let reader_handle = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let safe_rows = rows.max(2);
        let safe_cols = cols.max(2);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            safe_rows, safe_cols, 1_000_000,
        )));

        let command_state = Arc::new(Mutex::new(CommandState::Idle));

        let dirty = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(0));

        let parser_clone = Arc::clone(&parser);
        let cmd_state_clone = Arc::clone(&command_state);
        let dirty_clone = Arc::clone(&dirty);
        let output_at_clone = Arc::clone(&last_output_at);
        std::thread::spawn(move || {
            reader::read_pty_output(
                reader_handle,
                parser_clone,
                cmd_state_clone,
                dirty_clone,
                output_at_clone,
            );
        });

        debug!(url, rows, cols, "PTY instance spawned");

        Ok(Self {
            parser,
            writer: Some(writer),
            child: Some(child),
            rows,
            cols,
            master: Some(pair.master),
            scroll_offset: 0,
            name: String::new(),
            command_state,
            nvim_listen_addr: None,
            dirty,
            last_output_at,
        })
    }
}
