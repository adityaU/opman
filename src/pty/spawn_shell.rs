use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tracing::debug;

use super::reader;
use super::{CommandState, PtyInstance};

impl PtyInstance {
    /// Spawn a new PTY running the user's default shell.
    ///
    /// Uses $SHELL or falls back to /bin/bash.
    pub fn spawn_shell(
        rows: u16,
        cols: u16,
        working_dir: &std::path::Path,
        theme_envs: &[(String, String)],
        theme_dir: Option<&std::path::Path>,
        command: Option<&str>,
        name: Option<String>,
    ) -> Result<Self> {
        let shell = command
            .map(|s| s.to_string())
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/bash".to_string());

        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY pair for shell")?;

        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, val) in theme_envs {
            cmd.env(key, val);
        }
        if let Some(dir) = theme_dir {
            let gitui_theme = dir.join("gitui/opencode.ron");
            cmd.env("GITUI_THEME", gitui_theme.to_string_lossy().as_ref());

            if shell.contains("zsh") {
                let zdotdir = dir.join("zdotdir");
                if zdotdir.exists() {
                    if let Ok(orig) = std::env::var("ZDOTDIR") {
                        cmd.env("OPENCODE_ORIG_ZDOTDIR", &orig);
                    }
                    cmd.env("ZDOTDIR", zdotdir.to_string_lossy().as_ref());
                }
            } else if shell.contains("bash") {
                let bash_init = dir.join("bash_integration.sh");
                if bash_init.exists() {
                    cmd.env("BASH_ENV", bash_init.to_string_lossy().as_ref());
                }
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell in PTY")?;

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

        debug!(%shell, rows, cols, ?working_dir, "Shell PTY instance spawned");

        let pty = Self {
            parser,
            writer: Some(writer),
            child: Some(child),
            rows,
            cols,
            master: Some(pair.master),
            scroll_offset: 0,
            name: name.unwrap_or_else(|| String::new()),
            command_state,
            nvim_listen_addr: None,
            dirty,
            last_output_at,
        };
        Ok(pty)
    }
}
