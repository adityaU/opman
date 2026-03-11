use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::App;

pub(super) fn handle_slack_log_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.show_slack_log = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.slack_log_scroll = app.slack_log_scroll.saturating_add(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.slack_log_scroll = app.slack_log_scroll.saturating_sub(1);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_config_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.show_config_panel = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.config_panel_selected > 0 {
                app.config_panel_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = config_panel_setting_count() - 1;
            if app.config_panel_selected < max {
                app.config_panel_selected += 1;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            toggle_config_setting(app);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            adjust_config_setting(app, -5);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            adjust_config_setting(app, 5);
        }
        _ => {}
    }
    Ok(())
}

fn config_panel_setting_count() -> usize {
    4
}

fn toggle_config_setting(app: &mut App) {
    match app.config_panel_selected {
        0 => {
            app.config.settings.follow_edits_in_neovim =
                !app.config.settings.follow_edits_in_neovim;
        }
        2 => {
            app.config.settings.slack.enabled = !app.config.settings.slack.enabled;
        }
        _ => {}
    }
    if let Err(e) = app.config.save() {
        tracing::warn!("Failed to save config: {}", e);
    }
}

/// Adjust a numeric setting by `delta` (clamped to valid range).
/// Only applies to percentage-type or seconds-type settings; ignored for booleans.
fn adjust_config_setting(app: &mut App, delta: i16) {
    match app.config_panel_selected {
        1 => {
            let cur = app.config.settings.unfocused_dim_percent as i16;
            app.config.settings.unfocused_dim_percent =
                cur.saturating_add(delta).clamp(0, 100) as u8;
        }
        3 => {
            // Relay buffer: 1–60 seconds, step by 1 instead of 5.
            let step = if delta > 0 { 1i64 } else { -1i64 };
            let cur = app.config.settings.slack.relay_buffer_secs as i64;
            app.config.settings.slack.relay_buffer_secs =
                cur.saturating_add(step).clamp(1, 60) as u64;
        }
        _ => return,
    }
    if let Err(e) = app.config.save() {
        tracing::warn!("Failed to save config: {}", e);
    }
}
