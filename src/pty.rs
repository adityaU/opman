use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tracing::debug;

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

        let mut cmd = CommandBuilder::new("opencode");
        cmd.arg("attach");
        cmd.arg(url);
        cmd.arg("--dir");
        cmd.arg(working_dir);
        if let Some(sid) = session_id {
            cmd.arg("--session");
            cmd.arg(sid);
            // Export session ID as env var so MCP bridge processes (spawned by
            // opencode as children) can include it in socket requests, ensuring
            // terminal/neovim tool calls route to the correct session.
            cmd.env("OPENCODE_SESSION_ID", sid);
        }
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

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let safe_rows = rows.max(2);
        let safe_cols = cols.max(2);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(safe_rows, safe_cols, 10000)));

        let command_state = Arc::new(Mutex::new(CommandState::Idle));

        let parser_clone = Arc::clone(&parser);
        let cmd_state_clone = Arc::clone(&command_state);
        std::thread::spawn(move || {
            Self::read_pty_output(reader, parser_clone, cmd_state_clone);
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
        })
    }

    /// Background reader loop: reads bytes from the PTY and feeds them to the parser.
    ///
    /// Also scans for OSC 133 shell integration sequences to track command state:
    /// - `\x1b]133;B` → command started (Running)
    /// - `\x1b]133;D;0` → command succeeded (Success)
    /// - `\x1b]133;D;N` where N≠0 → command failed (Failure)
    /// - `\x1b]133;A` → prompt shown (Idle, resets after Success/Failure display)
    fn read_pty_output(
        mut reader: Box<dyn Read + Send>,
        parser: Arc<Mutex<vt100::Parser>>,
        command_state: Arc<Mutex<CommandState>>,
    ) {
        let mut buf = [0u8; 4096];
        let mut leftover: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = if leftover.is_empty() {
                        &buf[..n]
                    } else {
                        leftover.extend_from_slice(&buf[..n]);
                        leftover.as_slice()
                    };

                    Self::scan_osc133(data, &command_state);

                    if let Ok(mut p) = parser.lock() {
                        p.process(&buf[..n]);
                    }

                    // Keep trailing bytes for OSC sequences split across reads
                    let keep = data.len().min(32);
                    let tail = data[data.len() - keep..].to_vec();
                    leftover.clear();
                    leftover = tail;
                }
                Err(_) => break,
            }
        }
    }

    /// Scan a byte slice for OSC 133 shell integration sequences.
    ///
    /// Looks for patterns like `ESC ] 133 ; <cmd> BEL` or `ESC ] 133 ; <cmd> ST`
    /// where BEL = 0x07 and ST = ESC \.
    fn scan_osc133(data: &[u8], command_state: &Arc<Mutex<CommandState>>) {
        // OSC 133 prefix: ESC ] 1 3 3 ;
        const PREFIX: &[u8] = b"\x1b]133;";

        let mut pos = 0;
        while pos + PREFIX.len() < data.len() {
            if let Some(offset) = data[pos..].windows(PREFIX.len()).position(|w| w == PREFIX) {
                tracing::debug!("OSC 133 sequence detected in PTY output");
                let seq_start = pos + offset + PREFIX.len();
                if seq_start >= data.len() {
                    break;
                }

                match data[seq_start] {
                    // 'A' = prompt start → only reset to Idle if currently Running
                    // Keep Success/Failure sticky so dots remain visible until next command
                    b'A' => {
                        if let Ok(mut state) = command_state.lock() {
                            tracing::debug!(
                                "OSC 133;A (prompt start), current state: {:?}",
                                *state
                            );
                            if *state == CommandState::Running {
                                *state = CommandState::Idle;
                            }
                        }
                    }
                    b'B' => {
                        tracing::debug!("OSC 133;B (command start) → Running");
                        if let Ok(mut state) = command_state.lock() {
                            *state = CommandState::Running;
                        }
                    }
                    // 'C' = command output start (ignore, already Running)
                    b'C' => {}
                    b'D' => {
                        if seq_start + 1 < data.len() && data[seq_start + 1] == b';' {
                            let code_start = seq_start + 2;
                            let mut code_end = code_start;
                            while code_end < data.len() && data[code_end].is_ascii_digit() {
                                code_end += 1;
                            }
                            if code_end > code_start {
                                let code_str =
                                    std::str::from_utf8(&data[code_start..code_end]).unwrap_or("1");
                                let exit_code: i32 = code_str.parse().unwrap_or(1);
                                tracing::debug!(
                                    "OSC 133;D exit_code={} → {}",
                                    exit_code,
                                    if exit_code == 0 { "Success" } else { "Failure" }
                                );
                                if let Ok(mut state) = command_state.lock() {
                                    *state = if exit_code == 0 {
                                        CommandState::Success
                                    } else {
                                        CommandState::Failure
                                    };
                                }
                            } else {
                                tracing::debug!("OSC 133;D (no exit code) → Success");
                                if let Ok(mut state) = command_state.lock() {
                                    *state = CommandState::Success;
                                }
                            }
                        } else {
                            tracing::debug!("OSC 133;D (no semicolon) → Success");
                            if let Ok(mut state) = command_state.lock() {
                                *state = CommandState::Success;
                            }
                        }
                    }
                    _ => {}
                }

                pos = seq_start + 1;
            } else {
                break;
            }
        }
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

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let safe_rows = rows.max(2);
        let safe_cols = cols.max(2);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(safe_rows, safe_cols, 10000)));

        let command_state = Arc::new(Mutex::new(CommandState::Idle));

        let parser_clone = Arc::clone(&parser);
        let cmd_state_clone = Arc::clone(&command_state);
        std::thread::spawn(move || {
            Self::read_pty_output(reader, parser_clone, cmd_state_clone);
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
        };
        Ok(pty)
    }

    /// Spawn a new PTY running neovim.
    pub fn spawn_neovim(
        rows: u16,
        cols: u16,
        working_dir: &std::path::Path,
        theme_envs: &[(String, String)],
        theme_dir: Option<&std::path::Path>,
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

        // Compute a unique listen socket path per project.
        // Include both the manager PID and a hash of the working directory
        // so multiple projects each get their own neovim socket.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&working_dir, &mut hasher);
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

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let safe_rows = rows.max(2);
        let safe_cols = cols.max(2);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(safe_rows, safe_cols, 10000)));

        let command_state = Arc::new(Mutex::new(CommandState::Idle));

        let parser_clone = Arc::clone(&parser);
        let cmd_state_clone = Arc::clone(&command_state);
        std::thread::spawn(move || {
            Self::read_pty_output(reader, parser_clone, cmd_state_clone);
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
        })
    }

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

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let safe_rows = rows.max(2);
        let safe_cols = cols.max(2);
        let parser = Arc::new(Mutex::new(vt100::Parser::new(safe_rows, safe_cols, 10000)));

        let command_state = Arc::new(Mutex::new(CommandState::Idle));

        let parser_clone = Arc::clone(&parser);
        let cmd_state_clone = Arc::clone(&command_state);
        std::thread::spawn(move || {
            Self::read_pty_output(reader, parser_clone, cmd_state_clone);
        });

        debug!(rows, cols, ?working_dir, "GitUI PTY instance spawned");

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
        })
    }
}
