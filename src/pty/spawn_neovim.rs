use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tracing::debug;

use super::reader;
use super::{CommandState, PtyInstance};

impl PtyInstance {
    /// Spawn a new PTY running neovim.
    ///
    /// `session_id` is included in the socket path so that each session within
    /// the same project gets its own isolated neovim instance.
    pub fn spawn_neovim(
        rows: u16,
        cols: u16,
        working_dir: &std::path::Path,
        theme_envs: &[(String, String)],
        theme_dir: Option<&std::path::Path>,
        session_id: Option<&str>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY pair for neovim")?;

        let mut cmd = CommandBuilder::new("nvim");
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, val) in theme_envs {
            cmd.env(key, val);
        }

        // Compute a unique listen socket path per (project, session) pair.
        // Include the manager PID, a hash of the working directory, AND the
        // session ID so each session gets its own neovim socket — preventing
        // cross-session contamination when multiple sessions share a project.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&working_dir, &mut hasher);
        if let Some(sid) = session_id {
            std::hash::Hash::hash(sid, &mut hasher);
        }
        let dir_hash = std::hash::Hasher::finish(&hasher);
        let listen_path = std::path::PathBuf::from(format!(
            "/tmp/opencode-nvim-{}-{:x}.sock",
            std::process::id(),
            dir_hash
        ));
        // Clean up any stale socket from a previous run
        let _ = std::fs::remove_file(&listen_path);
        cmd.arg("--listen");
        cmd.arg(listen_path.to_string_lossy().as_ref());

        // Enable sign column and line numbers so diff signs are visible
        cmd.arg("--cmd");
        cmd.arg("set signcolumn=yes number norelativenumber noswapfile");
        if let Some(dir) = theme_dir {
            let colorscheme_path = dir.join("nvim/colors/opencode.lua");
            cmd.arg("--cmd");
            cmd.arg(format!(
                "autocmd VimEnter * ++once silent! luafile {}",
                colorscheme_path.display()
            ));
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn neovim in PTY")?;

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

        debug!(
            rows,
            cols,
            ?working_dir,
            ?listen_path,
            "Neovim PTY instance spawned"
        );

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
            nvim_listen_addr: Some(listen_path),
            dirty,
            last_output_at,
        })
    }
}
