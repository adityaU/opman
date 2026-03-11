use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use super::reader;
use super::{CommandState, PtyInstance};

impl PtyInstance {
    /// Spawn a new PTY running gitui.
    ///
    /// Uses the `-t` flag to load a custom theme file from the given path.
    pub fn spawn_gitui(
        rows: u16,
        cols: u16,
        working_dir: &std::path::Path,
        theme_path: Option<&std::path::Path>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY pair for gitui")?;

        let mut cmd = CommandBuilder::new("gitui");
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if let Some(theme) = theme_path {
            if theme.exists() {
                cmd.arg("-t");
                cmd.arg(theme.to_string_lossy().as_ref());
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn gitui in PTY")?;

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
