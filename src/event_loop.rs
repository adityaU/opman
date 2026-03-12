use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::app::{App, BackgroundEvent};
use crate::background_tasks::{spawn_session_fetch, spawn_session_select};
use crate::event_input;
use crate::event_mouse;
use crate::input::resize_ptys;
use crate::ui;
use crate::web;

/// The main event loop — polls for input and redraws the UI each tick.
/// Never blocks on network/process operations; all async work uses background tasks.
pub(crate) async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    watcher_rx: std::sync::mpsc::Receiver<notify::Event>,
    mut bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
    web_state_handle: Option<web::WebStateHandle>,
) -> Result<()> {
    let mut last_session_fetch = Instant::now();
    let mut last_theme_reload = Instant::now();
    let last_blink_toggle = Instant::now();
    // Previous pulse_phase value, used to detect changes worth redrawing.
    let mut prev_pulse_phase: f64 = 0.0;
    let mut last_countdown_redraw = Instant::now();
    // Track last toast message to detect new toasts for web broadcast.
    let mut prev_toast_msg: Option<String> = None;

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
        handle_pending_session_select(app);

        // ── 4.5. Handle pending new session (PTY without --session) ──
        handle_pending_new_session(app);

        // ── 5. (removed: gitui subprocess replaced by native git panel) ──

        // ── 6. Check for KV file changes (theme reload) ─────────────
        handle_kv_watcher(app, &watcher_rx, &mut last_theme_reload, &web_state_handle);

        // ── 7. Periodic session fetching (every 5 seconds, non-blocking) ──
        if last_session_fetch.elapsed() > Duration::from_secs(5) {
            spawn_session_fetch(&app.bg_tx, &app.projects);
            last_session_fetch = Instant::now();
        }

        // ── 7.5. Update pulse phase for active session dots ─────────
        update_pulse_phase(
            app,
            &last_blink_toggle,
            &mut prev_pulse_phase,
            &mut last_countdown_redraw,
        );

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

        // ── 7.8. Broadcast new toasts to web clients ──────────────────
        {
            let current_msg = app.toast_message.as_ref().map(|(m, _)| m.clone());
            if current_msg != prev_toast_msg {
                if let (Some(ref msg), Some(ref wsh)) = (&current_msg, &web_state_handle) {
                    wsh.broadcast_toast(msg.clone(), "info");
                }
                prev_toast_msg = current_msg;
            }
        }

        // ── 8. Poll for crossterm events (16ms tick = 60fps) ─────────
        // Drain ALL pending events before redrawing. When typing fast,
        // multiple keystrokes may arrive within one 16ms frame; process
        // them back-to-back so the PTY receives them without draw-cycle
        // latency between each keystroke.
        if event::poll(Duration::from_millis(16)).context("Event poll failed")? {
            loop {
                // Any crossterm event (key, mouse, paste, resize) means the UI
                // needs to update, so mark dirty unconditionally.
                app.needs_redraw = true;
                match event::read().context("Event read failed")? {
                    Event::Key(key) => {
                        event_input::handle_key_in_loop(app, key)?;
                    }
                    Event::Paste(text) => {
                        crate::input::handle_paste(app, &text);
                    }
                    Event::Resize(cols, rows) => {
                        let content_area =
                            ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
                        app.layout.compute_rects(content_area);
                        resize_ptys(app);
                    }
                    Event::Mouse(mouse_event) => {
                        event_mouse::handle_mouse_in_loop(app, mouse_event)?;
                    }
                    _ => {}
                }

                // Check for more pending events (non-blocking).
                // If none remain, break out so we redraw and process
                // background events before the next frame.
                if !event::poll(Duration::from_millis(0)).context("Event poll failed")? {
                    break;
                }
            } // loop (drain pending events)
        }
    }
    Ok(())
}

/// Handle pending session select from `app.pending_session_select`.
fn handle_pending_session_select(app: &mut App) {
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
}

/// Handle pending new session from `app.pending_new_session`.
fn handle_pending_new_session(app: &mut App) {
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
}

/// Drain KV file watcher events and reload theme if kv.json changed.
fn handle_kv_watcher(
    app: &mut App,
    watcher_rx: &std::sync::mpsc::Receiver<notify::Event>,
    last_theme_reload: &mut Instant,
    web_state_handle: &Option<web::WebStateHandle>,
) {
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
            *last_theme_reload = Instant::now();
            app.needs_redraw = true;

            // Broadcast theme change to web UI clients
            if let Some(ref wsh) = web_state_handle {
                let colors = web::WebThemeColors::from_theme(&app.theme);
                let wsh = wsh.clone();
                tokio::spawn(async move {
                    wsh.set_theme(colors).await;
                });
            }

            tracing::debug!("Theme reloaded from KV store change");
            continue;
        }
    }
}

/// Update pulse phase for active session dots and watcher countdowns.
fn update_pulse_phase(
    app: &mut App,
    last_blink_toggle: &Instant,
    prev_pulse_phase: &mut f64,
    last_countdown_redraw: &mut Instant,
) {
    let has_watcher_countdown = !app.watcher_idle_since.is_empty();
    let has_busy_watcher = app
        .active_sessions
        .iter()
        .any(|sid| app.session_watchers.contains_key(sid));
    if !app.active_sessions.is_empty() || has_watcher_countdown {
        let elapsed = last_blink_toggle.elapsed().as_secs_f64();
        // 1.5s full cycle, smooth sine wave clamped to 0.35..=1.0
        // so the dot never fades to background-color (invisible).
        let raw = (elapsed * std::f64::consts::PI / 0.75).sin().abs();
        let new_phase = 0.35 + raw * 0.65;
        // Only redraw when the rendered alpha actually changes
        // (quantize to 1/60th increments to avoid unnecessary redraws).
        if ((new_phase - *prev_pulse_phase) * 60.0).abs() >= 1.0 {
            app.pulse_phase = new_phase;
            *prev_pulse_phase = new_phase;
            app.needs_redraw = true;
        }
        // Force a redraw at least once per second during watcher
        // countdown so the elapsed/remaining seconds tick in the overlay.
        // Also force redraw for busy watched sessions so hang detection
        // timer updates in the overlay.
        if (has_watcher_countdown || has_busy_watcher)
            && last_countdown_redraw.elapsed() >= Duration::from_secs(1)
        {
            app.needs_redraw = true;
            *last_countdown_redraw = Instant::now();
        }
    }
}
