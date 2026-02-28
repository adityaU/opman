mod api;
mod app;
mod command_palette;
mod config;
mod input;
mod mcp;
mod mcp_neovim;
mod mcp_time;
mod nvim_rpc;
mod pty;
mod server;
mod sse;
mod theme;
mod theme_gen;
mod todo_db;
mod ui;
mod vim_mode;
mod which_key;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use input::resize_ptys;
use notify::{PollWatcher, RecursiveMode, Watcher};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tracing::info;

use crate::app::{App, BackgroundEvent};
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Log to file if OPENCODE_LOG_FILE is set, otherwise stderr
    let log_file_path = std::env::var("OPENCODE_LOG_FILE").ok();
    let log_writer: Box<dyn io::Write + Send> = if let Some(ref path) = log_file_path {
        Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .expect("Failed to open log file"),
        )
    } else {
        Box::new(io::stderr())
    };
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(log_writer))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("warn,notify=error,walkdir=error")
            }),
        )
        .init();

    // ── MCP bridge mode: `opman --mcp <project_path>` ─────
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--mcp" {
        let project_path = PathBuf::from(&args[2]);
        return mcp::run_mcp_bridge(project_path).await.map_err(Into::into);
    }

    // ── Time MCP bridge mode: `opman --mcp-time` ──────────
    if args.len() >= 2 && args[1] == "--mcp-time" {
        return mcp_time::run_mcp_time_bridge().await.map_err(Into::into);
    }

    // ── Neovim MCP bridge mode: `opman --mcp-nvim <project_path>` ──
    if args.len() >= 3 && args[1] == "--mcp-nvim" {
        let project_path = PathBuf::from(&args[2]);
        return mcp_neovim::run_mcp_neovim_bridge(project_path)
            .await
            .map_err(Into::into);
    }

    // ── Check for --no-* MCP flags ──────────────────────────────────
    let no_mcp = args.iter().any(|a| a == "--no-mcp");
    let enable_terminal_mcp = !no_mcp && !args.iter().any(|a| a == "--no-terminal-mcp");
    let enable_neovim_mcp = !no_mcp && !args.iter().any(|a| a == "--no-neovim-mcp");
    let enable_time_mcp = !no_mcp && !args.iter().any(|a| a == "--no-time-mcp");
    let enable_any_mcp = enable_terminal_mcp || enable_neovim_mcp || enable_time_mcp;

    info!("opman starting");

    // Spawn `opencode serve` on a free port before anything else
    let (base_url, server_handle) =
        server::spawn_opencode_server().context("Failed to start opencode serve")?;
    crate::app::init_base_url(base_url);

    // Kill the server on Ctrl+C (even if the TUI hasn't reached cleanup)
    {
        let handle = server_handle.clone();
        ctrlc::set_handler(move || {
            server::kill_server(&handle);
            std::process::exit(0);
        })
        .ok();
    }

    let config = Config::load().context("Failed to load config")?;

    // Deploy embedded opencode theme JSON files to ~/.config/opencode/themes/
    if let Err(e) = theme::deploy_embedded_themes() {
        tracing::warn!("Failed to deploy embedded themes: {}", e);
    }

    // 3. Create background event channel and app state
    let (bg_tx, bg_rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let mut app = App::new(config, bg_tx.clone());

    // Generate theme files for PTY programs (neovim, zsh, gitui)
    if let Err(e) = theme_gen::write_theme_files(&app.theme) {
        tracing::warn!("Failed to write theme files: {}", e);
    }

    // 4. Setup terminal — TUI renders IMMEDIATELY after this
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    stdout
        .execute(EnableMouseCapture)
        .context("Failed to enable mouse capture")?;
    stdout
        .execute(EnableBracketedPaste)
        .context("Failed to enable bracketed paste")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // 5. Setup KV file watcher for theme reloading
    let kv_path = {
        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/state")
            });
        state_dir.join("opencode/kv.json")
    };

    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();
    let poll_config = notify::Config::default().with_poll_interval(Duration::from_millis(500));
    let mut _watcher = PollWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = watcher_tx.send(event);
            }
        },
        poll_config,
    )
    .context("Failed to create file watcher")?;

    if kv_path.exists() {
        _watcher
            .watch(&kv_path, RecursiveMode::NonRecursive)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to watch KV file: {}", e);
            });
    } else if let Some(kv_dir) = kv_path.parent() {
        if kv_dir.exists() {
            _watcher
                .watch(kv_dir, RecursiveMode::NonRecursive)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to watch KV directory: {}", e);
                });
        }
    }

    // 6. Kick off initial data loading for all projects (shared server is already running externally)
    if !app.projects.is_empty() {
        // Fetch sessions for ALL projects immediately
        spawn_session_fetch(&app.bg_tx, &app.projects);

        // Start SSE listeners and session pollers for ALL projects
        for i in 0..app.projects.len() {
            let dir = app.projects[i].path.to_string_lossy().to_string();
            sse::spawn_sse_listener(&app.bg_tx, i, dir.clone());
            sse::spawn_session_poller(&app.bg_tx, i, dir.clone());
            sse::spawn_provider_fetcher(&app.bg_tx, i, dir);
        }

        // Spawn MCP socket servers and write opencode.json for each project
        if enable_any_mcp {
            for i in 0..app.projects.len() {
                let project_path = app.projects[i].path.clone();
                if enable_terminal_mcp || enable_neovim_mcp {
                    mcp::spawn_socket_server(&project_path, app.bg_tx.clone(), i);
                }
                if let Err(e) = mcp::write_opencode_json(
                    &project_path,
                    enable_terminal_mcp,
                    enable_neovim_mcp,
                    enable_time_mcp,
                ) {
                    tracing::warn!(
                        "Failed to write opencode.json for {}: {}",
                        project_path.display(),
                        e
                    );
                }
            }
        }

        // When neovim MCP is enabled, store flag on App (disables follow-edits)
        // and auto-start the neovim PTY for all projects so MCP tools work
        // immediately without requiring the user to open the neovim pane.
        if enable_neovim_mcp {
            app.neovim_mcp_enabled = true;
        }

        // Auto-activate project 0 by spawning PTY directly
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let content_area = ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
        app.layout.compute_rects(content_area);
        let (inner_cols, inner_rows) = app
            .layout
            .panel_rect(crate::ui::layout_manager::PanelId::TerminalPane)
            .map(|r| (r.width, r.height))
            .unwrap_or((cols.saturating_sub(32), rows.saturating_sub(2)));
        let path = app.projects[0].path.clone();
        let theme_envs = app.theme.pty_env_vars();
        spawn_activate_project(&app.bg_tx, 0, path, inner_rows, inner_cols, theme_envs);

        // Auto-start neovim PTY for all projects when neovim MCP is enabled,
        // so MCP tools work immediately without requiring the user to focus
        // the neovim pane. The pane itself remains hidden until manually opened.
        if enable_neovim_mcp {
            let saved = app.active_project;
            for i in 0..app.projects.len() {
                app.active_project = i;
                app.ensure_neovim_pty();
            }
            app.active_project = saved;
        }
    }

    // 7. Main event loop — TUI renders on first iteration (instant startup!)
    let result = run_event_loop(&mut terminal, &mut app, watcher_rx, bg_rx).await;

    // 8. Cleanup (always runs, even if event loop errored)
    disable_raw_mode().ok();
    terminal.backend_mut().execute(DisableBracketedPaste).ok();
    terminal.backend_mut().execute(DisableMouseCapture).ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    server::shutdown_all_ptys(&mut app.projects);
    server::kill_server(&server_handle);

    for child in app.popout_windows.drain(..) {
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
    }

    // Clean up MCP socket files
    if enable_any_mcp {
        for project in &app.projects {
            mcp::cleanup_socket(&project.path);
        }
    }

    info!("opman shut down");

    result
}

/// Spawn a background task to activate a project (PTY spawn).
/// Sends BackgroundEvent::PtySpawned on success.
fn spawn_activate_project(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_path: PathBuf,
    terminal_rows: u16,
    terminal_cols: u16,
    theme_envs: Vec<(String, String)>,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::task::spawn_blocking(move || {
        match pty::PtyInstance::spawn(
            &base_url,
            terminal_rows,
            terminal_cols,
            &project_path,
            None,
            &theme_envs,
        ) {
            Ok(pty) => {
                let _ = tx.send(BackgroundEvent::PtySpawned {
                    project_idx,
                    session_id: "__new__".to_string(),
                    pty,
                });
                let _ = tx.send(BackgroundEvent::ProjectActivated { project_idx });
            }
            Err(e) => {
                tracing::warn!(project_idx, "Background PTY spawn failed: {}", e);
            }
        }
    });
}

/// Spawn a background task to fetch sessions for all projects via the REST API.
fn spawn_session_fetch(bg_tx: &mpsc::UnboundedSender<BackgroundEvent>, projects: &[app::Project]) {
    let fetch_targets: Vec<(usize, String)> = projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let dir = p.path.to_string_lossy().to_string();
            (i, dir)
        })
        .collect();

    if fetch_targets.is_empty() {
        return;
    }

    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        for (project_idx, dir) in fetch_targets {
            match client.fetch_sessions(&base_url, &dir).await {
                Ok(sessions) => {
                    let _ = tx.send(BackgroundEvent::SessionsFetched {
                        project_idx,
                        sessions,
                    });
                }
                Err(_) => {
                    let _ = tx.send(BackgroundEvent::SessionFetchFailed { project_idx });
                }
            }
        }
    });
}

/// Spawn a background task to fetch sessions for a single project via the REST API.
fn spawn_single_session_fetch(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        match client.fetch_sessions(&base_url, &project_dir).await {
            Ok(sessions) => {
                let _ = tx.send(BackgroundEvent::SessionsFetched {
                    project_idx,
                    sessions,
                });
            }
            Err(_) => {
                let _ = tx.send(BackgroundEvent::SessionFetchFailed { project_idx });
            }
        }
    });
}

/// Spawn a background task to select a session via the API, then respawn PTY.
fn spawn_session_select(
    bg_tx: &mpsc::UnboundedSender<BackgroundEvent>,
    project_idx: usize,
    project_dir: String,
    session_id: String,
    project_path: PathBuf,
    terminal_rows: u16,
    terminal_cols: u16,
    theme_envs: Vec<(String, String)>,
) {
    let tx = bg_tx.clone();
    let base_url = crate::app::base_url().to_string();
    tokio::spawn(async move {
        let client = api::ApiClient::new();
        let _ = client
            .select_session(&base_url, &project_dir, &session_id)
            .await;
        let sid_for_pty = session_id.clone();
        let _ = tx.send(BackgroundEvent::SessionSelected {
            project_idx,
            session_id,
        });

        // Respawn PTY attached to the newly selected session
        let tx2 = tx.clone();
        let url = base_url.clone();
        let path = project_path.clone();
        let sid_clone = sid_for_pty.clone();
        tokio::task::spawn_blocking(move || {
            match pty::PtyInstance::spawn(
                &url,
                terminal_rows,
                terminal_cols,
                &path,
                Some(&sid_for_pty),
                &theme_envs,
            ) {
                Ok(pty) => {
                    let _ = tx2.send(BackgroundEvent::PtySpawned {
                        project_idx,
                        session_id: sid_clone,
                        pty,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        project_idx,
                        "PTY respawn after session select failed: {}",
                        e
                    );
                }
            }
        })
        .await
        .ok();
    });
}

/// The main event loop — polls for input and redraws the UI each tick.
/// Never blocks on network/process operations; all async work uses background tasks.
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    watcher_rx: std::sync::mpsc::Receiver<notify::Event>,
    mut bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<()> {
    let mut last_session_fetch = Instant::now();
    let mut last_theme_reload = Instant::now();
    let last_blink_toggle = Instant::now();
    // Previous pulse_phase value, used to detect changes worth redrawing.
    let mut prev_pulse_phase: f64 = 0.0;

    loop {
        // ── 1. Draw the UI only when something actually changed ──────
        // Collect PTY dirty flags (new terminal output from reader threads).
        let any_pty_dirty = app.drain_pty_dirty_flags();
        if app.needs_redraw || any_pty_dirty {
            app.sync_sidebar_to_active_session();
            terminal.draw(|frame| ui::draw(frame, app))?;
            app.needs_redraw = false;
        }

        if app.should_quit {
            break;
        }

        // ── 2. Drain ALL background events (zero-cost when empty) ────
        while let Ok(event) = bg_rx.try_recv() {
            app.handle_background_event(event);
            app.needs_redraw = true;
        }

        // ── 3. Handle pending project removal ────────────────────────
        if let Some(idx) = app.pending_remove.take() {
            app.remove_project(idx)?;
            app.needs_redraw = true;
        }

        // ── 4. Handle pending session select (non-blocking) ──────────
        if let Some((proj_idx, session_id)) = app.pending_session_select.take() {
            app.needs_redraw = true;
            if let Some(project) = app.projects.get(proj_idx) {
                if project.ptys.contains_key(&session_id) {
                    app.projects[proj_idx].active_session = Some(session_id.clone());
                    let dir = app.projects[proj_idx].path.to_string_lossy().to_string();
                    let sid = session_id.clone();
                    let base_url = crate::app::base_url().to_string();
                    tokio::spawn(async move {
                        let client = crate::api::ApiClient::new();
                        let _ = client.select_session(&base_url, &dir, &sid).await;
                    });
                } else {
                    let dir = project.path.to_string_lossy().to_string();
                    let project_path = project.path.clone();
                    let (inner_cols, inner_rows) = app
                        .layout
                        .panel_rect(crate::ui::layout_manager::PanelId::TerminalPane)
                        .map(|r| (r.width, r.height))
                        .unwrap_or((80, 24));
                    let theme_envs = app.theme.pty_env_vars();
                    spawn_session_select(
                        &app.bg_tx,
                        proj_idx,
                        dir,
                        session_id,
                        project_path,
                        inner_rows,
                        inner_cols,
                        theme_envs,
                    );
                }
            }
        }

        // ── 4.5. Handle pending new session (PTY without --session) ──
        // pending_new_session is consumed once to spawn PTY.
        // awaiting_new_session persists for the SSE handler to auto-select.
        if let Some(proj_idx) = app.pending_new_session.take() {
            app.needs_redraw = true;
            app.awaiting_new_session = Some(proj_idx);
            if let Some(project) = app.projects.get(proj_idx) {
                let project_path = project.path.clone();
                let (inner_cols, inner_rows) = app
                    .layout
                    .panel_rect(crate::ui::layout_manager::PanelId::TerminalPane)
                    .map(|r| (r.width, r.height))
                    .unwrap_or((80, 24));
                let bg_tx = app.bg_tx.clone();
                let base_url = crate::app::base_url().to_string();
                let theme_envs = app.theme.pty_env_vars();
                tokio::spawn(async move {
                    let idx = proj_idx;
                    match tokio::task::spawn_blocking(move || {
                        crate::pty::PtyInstance::spawn(
                            &base_url,
                            inner_rows,
                            inner_cols,
                            &project_path,
                            None,
                            &theme_envs,
                        )
                    })
                    .await
                    {
                        Ok(Ok(pty)) => {
                            let _ = bg_tx.send(BackgroundEvent::PtySpawned {
                                project_idx: idx,
                                session_id: "__new__".to_string(),
                                pty,
                            });
                        }
                        Ok(Err(e)) => {
                            tracing::error!("Failed to spawn new session PTY: {e}");
                        }
                        Err(e) => {
                            tracing::error!("New session PTY task panicked: {e}");
                        }
                    }
                });
            }
        }

        // ── 5. (removed: gitui subprocess replaced by native git panel) ──

        // ── 6. Check for KV file changes (theme reload) ─────────────
        while let Ok(event) = watcher_rx.try_recv() {
            let is_kv_change = event
                .paths
                .iter()
                .any(|p| p.file_name().map(|f| f == "kv.json").unwrap_or(false));

            if is_kv_change && last_theme_reload.elapsed() > Duration::from_millis(500) {
                app.theme = crate::theme::load_theme();
                if let Err(e) = crate::theme_gen::write_theme_files(&app.theme) {
                    tracing::warn!("Failed to regenerate theme files: {e}");
                }
                app.update_ptys_for_theme();
                last_theme_reload = Instant::now();
                app.needs_redraw = true;
                tracing::debug!("Theme reloaded from KV store change");
                continue;
            }
        }

        // ── 7. Periodic session fetching (every 5 seconds, non-blocking) ──
        if last_session_fetch.elapsed() > Duration::from_secs(5) {
            spawn_session_fetch(&app.bg_tx, &app.projects);
            last_session_fetch = Instant::now();
        }

        // ── 7.5. Update pulse phase for active session dots ─────────
        if !app.active_sessions.is_empty() {
            let elapsed = last_blink_toggle.elapsed().as_secs_f64();
            // 1.5s full cycle, smooth sine wave clamped to 0.35..=1.0
            // so the dot never fades to background-color (invisible).
            let raw = (elapsed * std::f64::consts::PI / 0.75).sin().abs();
            let new_phase = 0.35 + raw * 0.65;
            // Only redraw when the rendered alpha actually changes
            // (quantize to 1/60th increments to avoid unnecessary redraws).
            if ((new_phase - prev_pulse_phase) * 60.0).abs() >= 1.0 {
                app.pulse_phase = new_phase;
                prev_pulse_phase = new_phase;
                app.needs_redraw = true;
            }
        }

        // ── 7.6. Tick fuzzy picker matcher (processes walker results) ─
        if let Some(ref mut picker) = app.fuzzy_picker {
            if picker.tick() {
                app.needs_redraw = true;
            }
        }

        // ── 7.7. Clear expired toast notifications ─────────────────────
        if let Some((_, ts)) = &app.toast_message {
            if ts.elapsed() > std::time::Duration::from_secs(2) {
                app.toast_message = None;
                app.needs_redraw = true;
            }
        }

        // ── 8. Poll for crossterm events (16ms tick = 60fps) ─────────
        if event::poll(Duration::from_millis(16)).context("Event poll failed")? {
            // Any crossterm event (key, mouse, paste, resize) means the UI
            // needs to update, so mark dirty unconditionally.
            app.needs_redraw = true;
            match event::read().context("Event read failed")? {
                Event::Key(key) => {
                    // Terminal search input handling — intercept keys before normal handler
                    if app.terminal_search.is_some() {
                        use crossterm::event::{KeyCode, KeyModifiers};
                        let handled = match key.code {
                            KeyCode::Esc => {
                                app.terminal_search = None;
                                true
                            }
                            KeyCode::Enter => {
                                // Next match
                                if let Some(ref mut search) = app.terminal_search {
                                    if !search.matches.is_empty() {
                                        search.current_match =
                                            (search.current_match + 1) % search.matches.len();
                                        // Scroll to match
                                        let (match_row, _, _) =
                                            search.matches[search.current_match];
                                        if let Some(project) =
                                            app.projects.get_mut(app.active_project)
                                        {
                                            if let Some(pty) = project.active_shell_pty_mut() {
                                                // Count lines and set scrollback in one lock
                                                // to avoid clone + double-lock.
                                                if let Ok(mut p) = pty.parser.lock() {
                                                    let rows = p.screen().size().0 as usize;
                                                    let total =
                                                        p.screen().contents().lines().count();
                                                    if match_row + rows < total {
                                                        pty.scroll_offset =
                                                            total - match_row - rows;
                                                        p.set_scrollback(pty.scroll_offset);
                                                    } else {
                                                        pty.scroll_offset = 0;
                                                        p.set_scrollback(0);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                true
                            }
                            KeyCode::Backspace => {
                                if let Some(ref mut search) = app.terminal_search {
                                    search.query.pop();
                                    search.cursor = search.query.len();
                                    // Re-run search
                                    update_terminal_search_matches(app);
                                }
                                true
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'n' {
                                    // Ctrl+N = next match
                                    if let Some(ref mut search) = app.terminal_search {
                                        if !search.matches.is_empty() {
                                            search.current_match =
                                                (search.current_match + 1) % search.matches.len();
                                        }
                                    }
                                    true
                                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p'
                                {
                                    // Ctrl+P = prev match
                                    if let Some(ref mut search) = app.terminal_search {
                                        if !search.matches.is_empty() {
                                            search.current_match = if search.current_match == 0 {
                                                search.matches.len().saturating_sub(1)
                                            } else {
                                                search.current_match - 1
                                            };
                                        }
                                    }
                                    true
                                } else if key.modifiers.is_empty()
                                    || key.modifiers == KeyModifiers::SHIFT
                                {
                                    // Regular character input
                                    if let Some(ref mut search) = app.terminal_search {
                                        search.query.push(c);
                                        search.cursor = search.query.len();
                                    }
                                    update_terminal_search_matches(app);
                                    true
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };

                        if handled {
                            continue;
                        }
                    }

                    // Shift+PgUp/PgDn for terminal keyboard scrollback
                    {
                        use crossterm::event::{KeyCode, KeyModifiers};
                        let is_terminal_focused = matches!(
                            app.layout.focused,
                            crate::ui::layout_manager::PanelId::IntegratedTerminal
                        );
                        if is_terminal_focused {
                            match (key.code, key.modifiers.contains(KeyModifiers::SHIFT)) {
                                (KeyCode::PageUp, true) => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some(pty) = project.active_shell_pty_mut() {
                                            pty.scroll_offset = pty
                                                .scroll_offset
                                                .saturating_add(pty.rows as usize / 2);
                                            if let Ok(mut p) = pty.parser.lock() {
                                                p.set_scrollback(pty.scroll_offset);
                                                pty.scroll_offset = p.screen().scrollback();
                                            }
                                        }
                                    }
                                    continue;
                                }
                                (KeyCode::PageDown, true) => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some(pty) = project.active_shell_pty_mut() {
                                            pty.scroll_offset = pty
                                                .scroll_offset
                                                .saturating_sub(pty.rows as usize / 2);
                                            if let Ok(mut p) = pty.parser.lock() {
                                                p.set_scrollback(pty.scroll_offset);
                                                pty.scroll_offset = p.screen().scrollback();
                                            }
                                        }
                                    }
                                    continue;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Track active project before key handling
                    let pre_active = app.active_project;
                    input::handle_key_event(app, key)?;

                    // If active project changed via sidebar Enter, activate in background
                    if app.active_project != pre_active {
                        let new_idx = app.active_project;

                        // Fetch sessions immediately for the newly switched project
                        if let Some(p) = app.projects.get(new_idx) {
                            let dir = p.path.to_string_lossy().to_string();
                            spawn_single_session_fetch(&app.bg_tx, new_idx, dir);
                        }

                        // Spawn PTY in background if needed
                        if app
                            .projects
                            .get(new_idx)
                            .map(|p| p.ptys.is_empty())
                            .unwrap_or(false)
                        {
                            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                            let content_area =
                                ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
                            app.layout.compute_rects(content_area);
                            let (inner_cols, inner_rows) = app
                                .layout
                                .panel_rect(crate::ui::layout_manager::PanelId::TerminalPane)
                                .map(|r| (r.width, r.height))
                                .unwrap_or((cols.saturating_sub(32), rows.saturating_sub(2)));
                            let path = app.projects[new_idx].path.clone();
                            let theme_envs = app.theme.pty_env_vars();
                            spawn_activate_project(
                                &app.bg_tx, new_idx, path, inner_rows, inner_cols, theme_envs,
                            );
                        }
                    }
                }
                Event::Paste(text) => {
                    input::handle_paste(app, &text);
                }
                Event::Resize(cols, rows) => {
                    let content_area =
                        ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
                    app.layout.compute_rects(content_area);
                    resize_ptys(app);
                }
                Event::Mouse(mouse_event) => {
                    let (cols, rows) = crossterm::terminal::size()?;
                    let area = ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));

                    app.layout.compute_rects(area);
                    let dragging = app.layout.handle_mouse(mouse_event, area);
                    if dragging.is_some() {
                        app.layout.compute_rects(area);
                        resize_ptys(app);
                    }

                    // Forward mouse events to the PTY if the mouse is over a terminal panel
                    if dragging.is_none() {
                        // In zen mode, only the focused panel is rendered full-screen,
                        // but panel_at() still uses non-zen layout rects → wrong panel.
                        // Short-circuit: always use the focused panel in zen mode.
                        let panel_opt = if app.zen_mode {
                            Some(app.layout.focused)
                        } else {
                            app.layout.panel_at(mouse_event.column, mouse_event.row)
                        };
                        if let Some(panel) = panel_opt {
                            match panel {
                                crate::ui::layout_manager::PanelId::Sidebar => {
                                    if let crossterm::event::MouseEventKind::Down(
                                        crossterm::event::MouseButton::Left,
                                    ) = mouse_event.kind
                                    {
                                        if let Some(rect) = app
                                            .layout
                                            .panel_rect(crate::ui::layout_manager::PanelId::Sidebar)
                                        {
                                            let relative_y =
                                                mouse_event.row.saturating_sub(rect.y) as usize;
                                            let item_count = app.sidebar_item_count();
                                            if relative_y < item_count {
                                                app.sidebar_cursor = relative_y;
                                                app.layout.focused =
                                                    crate::ui::layout_manager::PanelId::Sidebar;
                                                if let Some(item) = app.sidebar_item_at(relative_y)
                                                {
                                                    match item {
                                                        crate::app::SidebarItem::Project(idx) => {
                                                            if app.active_project != idx {
                                                                app.switch_project(idx);
                                                            }
                                                            if app.sessions_expanded_for == Some(idx) {
                                                                app.sessions_expanded_for = None;
                                                            } else {
                                                                app.sessions_expanded_for = Some(idx);
                                                            }
                                                        }
                                                        crate::app::SidebarItem::NewSession(proj_idx) => {
                                                            if app.active_project != proj_idx {
                                                                app.switch_project(proj_idx);
                                                            }
                                                            if let Some(project) = app.projects.get_mut(proj_idx) {
                                                                if let Some(pty) = project.active_pty_mut() {
                                                                    let _ = pty.kill();
                                                                }
                                                                if let Some(sid) = project.active_session.take() {
                                                                    project.ptys.remove(&sid);
                                                                }
                                                            }
                                                            app.pending_new_session = Some(proj_idx);
                                                            app.pending_session_select = None;
                                                            app.layout.focused = crate::ui::layout_manager::PanelId::TerminalPane;
                                                        }
                                                        crate::app::SidebarItem::Session(proj_idx, session_id) => {
                                                            if app.active_project != proj_idx {
                                                                app.switch_project(proj_idx);
                                                            }
                                                            // Check if click is on the arrow (▶/▼) column to toggle subagents
                                                            let relative_x = mouse_event.column.saturating_sub(rect.x) as usize;
                                                            let has_subagents = !app.subagent_sessions(proj_idx, &session_id).is_empty();
                                                            // Arrow sits at columns 6-9 ("    └ " = 6, optional "● " = +2, arrow "▶ " = 2)
                                                            if has_subagents && relative_x >= 6 && relative_x <= 9 {
                                                                if app.subagents_expanded_for.as_deref() == Some(&session_id) {
                                                                    app.subagents_expanded_for = None;
                                                                } else {
                                                                    app.subagents_expanded_for = Some(session_id);
                                                                }
                                                            } else {
                                                                if let Some(project) = app.projects.get(proj_idx) {
                                                                    if project.ptys.contains_key(&session_id) {
                                                                        app.projects[proj_idx].active_session = Some(session_id.clone());
                                                                        app.active_project = proj_idx;
                                                                        let dir = app.projects[proj_idx].path.to_string_lossy().to_string();
                                                                        let sid = session_id.clone();
                                                                        let base_url = crate::app::base_url().to_string();
                                                                        tokio::spawn(async move {
                                                                            let client = crate::api::ApiClient::new();
                                                                            let _ = client.select_session(&base_url, &dir, &sid).await;
                                                                        });
                                                                    } else {
                                                                        app.pending_session_select = Some((proj_idx, session_id));
                                                                    }
                                                                }
                                                                app.layout.focused = crate::ui::layout_manager::PanelId::TerminalPane;
                                                            }
                                                        }
                                                        crate::app::SidebarItem::MoreSessions(proj_idx) => {
                                                            if app.active_project != proj_idx {
                                                                app.switch_project(proj_idx);
                                                            }
                                                            app.open_session_search();
                                                        }
                                                        crate::app::SidebarItem::SubAgentSession(proj_idx, session_id) => {
                                                            if app.active_project != proj_idx {
                                                                app.switch_project(proj_idx);
                                                            }
                                                            if let Some(project) = app.projects.get(proj_idx) {
                                                                if project.ptys.contains_key(&session_id) {
                                                                    app.projects[proj_idx].active_session = Some(session_id.clone());
                                                                    app.active_project = proj_idx;
                                                                    let dir = app.projects[proj_idx].path.to_string_lossy().to_string();
                                                                    let sid = session_id.clone();
                                                                    let base_url = crate::app::base_url().to_string();
                                                                    tokio::spawn(async move {
                                                                        let client = crate::api::ApiClient::new();
                                                                        let _ = client.select_session(&base_url, &dir, &sid).await;
                                                                    });
                                                                } else {
                                                                    app.pending_session_select = Some((proj_idx, session_id));
                                                                }
                                                            }
                                                            app.layout.focused = crate::ui::layout_manager::PanelId::TerminalPane;
                                                        }
                                                        crate::app::SidebarItem::AddProject => {
                                                            app.start_add_project();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                crate::ui::layout_manager::PanelId::TerminalPane => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some((pty, rect)) =
                                            project.active_pty_mut().zip(app.layout.panel_rect(
                                                crate::ui::layout_manager::PanelId::TerminalPane,
                                            ))
                                        {
                                            forward_mouse_to_pty(
                                                pty,
                                                &mouse_event,
                                                rect.x,
                                                rect.y,
                                                crate::ui::layout_manager::PanelId::TerminalPane,
                                                &mut app.terminal_selection,
                                                &mut app.toast_message,
                                            );
                                        }
                                    }
                                }
                                crate::ui::layout_manager::PanelId::IntegratedTerminal => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some(rect) = app.layout.panel_rect(
                                            crate::ui::layout_manager::PanelId::IntegratedTerminal,
                                        ) {
                                            let has_tab_bar = project
                                                .active_resources()
                                                .map(|r| r.shell_ptys.len())
                                                .unwrap_or(0)
                                                > 1;
                                            // Check if click is on the tab bar row
                                            if has_tab_bar && mouse_event.row == rect.y {
                                                if matches!(
                                                    mouse_event.kind,
                                                    crossterm::event::MouseEventKind::Down(
                                                        crossterm::event::MouseButton::Left
                                                    )
                                                ) {
                                                    // Calculate which tab was clicked based on x position
                                                    let click_x =
                                                        mouse_event.column.saturating_sub(rect.x)
                                                            as usize;
                                                    let mut x_offset = 0usize;
                                                    let mut clicked_tab = None;
                                                    if let Some(resources) =
                                                        project.active_resources()
                                                    {
                                                        for (i, pty) in
                                                            resources.shell_ptys.iter().enumerate()
                                                        {
                                                            let label = if pty.name.is_empty() {
                                                                format!(" Tab {} ", i + 1)
                                                            } else {
                                                                format!(" {} ", pty.name)
                                                            };
                                                            let label_len = label.len();
                                                            // Include command-state dot width to match rendering
                                                            let cmd_state = pty
                                                                .command_state
                                                                .lock()
                                                                .unwrap()
                                                                .clone();
                                                            let dot_width = if cmd_state
                                                                != crate::pty::CommandState::Idle
                                                            {
                                                                1
                                                            } else {
                                                                0
                                                            };
                                                            if click_x >= x_offset
                                                                && click_x
                                                                    < x_offset
                                                                        + label_len
                                                                        + dot_width
                                                            {
                                                                clicked_tab = Some(i);
                                                                break;
                                                            }
                                                            x_offset += label_len + dot_width;
                                                        }
                                                    }
                                                    if let Some(tab) = clicked_tab {
                                                        if let Some(resources) =
                                                            project.active_resources_mut()
                                                        {
                                                            resources.active_shell_tab = tab;
                                                        }
                                                    }
                                                }
                                            } else if let Some(pty) = project.active_shell_pty_mut()
                                            {
                                                // Account for tab bar offset only when tab bar is visible
                                                let content_offset_y =
                                                    if has_tab_bar { rect.y + 1 } else { rect.y };
                                                forward_mouse_to_pty(
                                                    pty,
                                                    &mouse_event,
                                                    rect.x,
                                                    content_offset_y,
                                                    crate::ui::layout_manager::PanelId::IntegratedTerminal,
                                                    &mut app.terminal_selection,
                                                    &mut app.toast_message,
                                                );
                                            }
                                        }
                                    }
                                }
                                crate::ui::layout_manager::PanelId::NeovimPane => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some((pty, rect)) = project
                                            .active_resources_mut()
                                            .and_then(|r| r.neovim_pty.as_mut())
                                            .zip(app.layout.panel_rect(
                                                crate::ui::layout_manager::PanelId::NeovimPane,
                                            ))
                                        {
                                            forward_mouse_to_pty(
                                                pty,
                                                &mouse_event,
                                                rect.x,
                                                rect.y,
                                                crate::ui::layout_manager::PanelId::NeovimPane,
                                                &mut app.terminal_selection,
                                                &mut app.toast_message,
                                            );
                                        }
                                    }
                                }
                                crate::ui::layout_manager::PanelId::GitPanel => {
                                    if let Some(project) = app.projects.get_mut(app.active_project)
                                    {
                                        if let Some((pty, rect)) =
                                            project.gitui_pty.as_mut().zip(app.layout.panel_rect(
                                                crate::ui::layout_manager::PanelId::GitPanel,
                                            ))
                                        {
                                            forward_mouse_to_pty(
                                                pty,
                                                &mouse_event,
                                                rect.x,
                                                rect.y,
                                                crate::ui::layout_manager::PanelId::GitPanel,
                                                &mut app.terminal_selection,
                                                &mut app.toast_message,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Forward mouse events to a PTY.
/// Mouse mode active (vim, less, htop, opencode): forward as SGR bytes.
/// Mouse mode off (plain shell): scroll adjusts scrollback offset for viewing history.
fn forward_mouse_to_pty(
    pty: &mut crate::pty::PtyInstance,
    event: &crossterm::event::MouseEvent,
    panel_x: u16,
    panel_y: u16,
    panel_id: crate::ui::layout_manager::PanelId,
    terminal_selection: &mut Option<crate::app::TerminalSelection>,
    toast_message: &mut Option<(String, std::time::Instant)>,
) {
    use crossterm::event::{MouseButton, MouseEventKind};

    // Acquire lock once to check mouse mode and handle scroll in one shot.
    // This reduces lock acquisitions from 2-3 per scroll event down to 1.
    let mouse_mode = {
        let mut parser = match pty.parser.lock() {
            Ok(p) => p,
            Err(_) => return,
        };

        let mode = parser.screen().mouse_protocol_mode();

        if mode == vt100::MouseProtocolMode::None {
            // Handle scroll events while we already hold the lock
            match event.kind {
                MouseEventKind::ScrollUp => {
                    pty.scroll_offset = pty.scroll_offset.saturating_add(3);
                    parser.set_scrollback(pty.scroll_offset);
                    pty.scroll_offset = parser.screen().scrollback();
                    return;
                }
                MouseEventKind::ScrollDown => {
                    pty.scroll_offset = pty.scroll_offset.saturating_sub(3);
                    parser.set_scrollback(pty.scroll_offset);
                    pty.scroll_offset = parser.screen().scrollback();
                    return;
                }
                _ => {} // fall through, lock will be dropped
            }
        }

        mode
    };
    // Lock is now released.

    if mouse_mode == vt100::MouseProtocolMode::None {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let rel_col = event.column.saturating_sub(panel_x);
                let rel_row = event.row.saturating_sub(panel_y);

                // Ctrl+Click: open URL at cursor position
                if event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    // Clone screen and drop lock before doing string work
                    let row_text = if let Ok(parser) = pty.parser.lock() {
                        let screen = parser.screen();
                        screen.contents_between(rel_row, 0, rel_row, screen.size().1 - 1)
                    } else {
                        return;
                    };
                    // Lock released — do URL scanning without holding it
                    let prefixes = ["https://", "http://", "ftp://"];
                    let end_chars: &[char] = &[' ', '\t', '"', '\'', '>', '<', ')', ']', '}', '|'];
                    for prefix in &prefixes {
                        let mut search_from = 0usize;
                        while let Some(start) = row_text[search_from..].find(prefix) {
                            let abs_start = search_from + start;
                            let url_end = row_text[abs_start..]
                                .find(|c: char| end_chars.contains(&c) || c.is_control())
                                .map(|e| abs_start + e)
                                .unwrap_or(row_text.trim_end().len());
                            let col = rel_col as usize;
                            if col >= abs_start && col < url_end {
                                let url = &row_text[abs_start..url_end];
                                let _ = std::process::Command::new("open").arg(url).spawn();
                                return;
                            }
                            search_from = url_end;
                        }
                    }
                }

                // Start text selection
                *terminal_selection = Some(crate::app::TerminalSelection {
                    panel_id,
                    start_row: rel_row,
                    start_col: rel_col,
                    end_row: rel_row,
                    end_col: rel_col,
                });
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Update selection end
                if let Some(ref mut sel) = terminal_selection {
                    if sel.panel_id == panel_id {
                        sel.end_row = event.row.saturating_sub(panel_y);
                        sel.end_col = event.column.saturating_sub(panel_x);
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Extract text and copy to clipboard
                if let Some(ref sel) = terminal_selection {
                    if sel.panel_id == panel_id {
                        if let Ok(parser) = pty.parser.lock() {
                            let screen = parser.screen().clone();
                            drop(parser);

                            let (sr, sc, er, ec) =
                                if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
                                    (sel.start_row, sel.start_col, sel.end_row, sel.end_col)
                                } else {
                                    (sel.end_row, sel.end_col, sel.start_row, sel.start_col)
                                };

                            let text = screen.contents_between(sr, sc, er, ec);

                            use std::io::Write;
                            use std::process::{Command, Stdio};
                            if !text.trim().is_empty() {
                                if let Ok(mut child) =
                                    Command::new("pbcopy").stdin(Stdio::piped()).spawn()
                                {
                                    if let Some(stdin) = child.stdin.as_mut() {
                                        let _ = stdin.write_all(text.as_bytes());
                                    }
                                    let _ = child.wait();
                                }
                                *toast_message =
                                    Some(("Copied!".to_string(), std::time::Instant::now()));
                            }
                        }
                        *terminal_selection = None;
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // Mouse mode active — reset scrollback and forward SGR bytes
    if pty.scroll_offset > 0 {
        pty.scroll_offset = 0;
        if let Ok(mut p) = pty.parser.lock() {
            p.set_scrollback(0);
        }
    }
    if let Some(bytes) = input::mouse_event_to_bytes(event, panel_x, panel_y) {
        let _ = pty.write(&bytes);
    }
}

/// Update terminal search matches based on current query.
/// Searches the full terminal buffer (scrollback + visible) for the query string.
fn update_terminal_search_matches(app: &mut app::App) {
    let query = if let Some(ref search) = app.terminal_search {
        if search.query.is_empty() {
            app.terminal_search.as_mut().unwrap().matches.clear();
            app.terminal_search.as_mut().unwrap().current_match = 0;
            return;
        }
        search.query.to_lowercase()
    } else {
        return;
    };

    let mut matches = Vec::new();
    if let Some(project) = app.projects.get(app.active_project) {
        if let Some(pty) = project.active_shell_pty() {
            if let Ok(parser) = pty.parser.lock() {
                // Only iterates visible rows (typically 24-50), so fast under lock.
                let screen = parser.screen();
                let rows = screen.size().0;
                for row_idx in 0..rows {
                    let row_text = screen
                        .contents_between(row_idx, 0, row_idx + 1, 0)
                        .to_lowercase();
                    let mut search_from = 0;
                    while let Some(col) = row_text[search_from..].find(&query) {
                        let actual_col = search_from + col;
                        matches.push((row_idx as usize, actual_col, query.len()));
                        search_from = actual_col + 1;
                    }
                }
            }
        }
    }

    if let Some(ref mut search) = app.terminal_search {
        search.matches = matches;
        if search.current_match >= search.matches.len() {
            search.current_match = 0;
        }
    }
}
