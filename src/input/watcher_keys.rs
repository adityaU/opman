use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

use super::watcher;

pub(super) fn handle_watcher_session_list_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut m) = app.watcher_modal {
                if m.selected_session_idx > 0 {
                    m.selected_session_idx -= 1;
                    let sid = m.sessions[m.selected_session_idx].session_id.clone();
                    let pidx = m.sessions[m.selected_session_idx].project_idx;
                    watcher::fetch_watcher_session_messages(app, &sid, pidx);
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut m) = app.watcher_modal {
                if m.selected_session_idx + 1 < m.sessions.len() {
                    m.selected_session_idx += 1;
                    let sid = m.sessions[m.selected_session_idx].session_id.clone();
                    let pidx = m.sessions[m.selected_session_idx].project_idx;
                    watcher::fetch_watcher_session_messages(app, &sid, pidx);
                }
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            // Remove watcher for selected session
            if let Some(ref m) = app.watcher_modal {
                if let Some(entry) = m.sessions.get(m.selected_session_idx) {
                    if entry.has_watcher {
                        let sid = entry.session_id.clone();
                        app.session_watchers.remove(&sid);
                        app.watcher_idle_since.remove(&sid);
                        // Cancel pending timer if any
                        if let Some(handle) = app.watcher_pending.remove(&sid) {
                            handle.abort();
                        }
                        // Update the entry's has_watcher flag
                        if let Some(ref mut m) = app.watcher_modal {
                            if let Some(entry) = m.sessions.get_mut(m.selected_session_idx) {
                                entry.has_watcher = false;
                            }
                        }
                        app.toast_message =
                            Some(("Watcher removed".into(), std::time::Instant::now()));
                    }
                }
            }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            // Switch focus to right panel
            if let Some(ref mut m) = app.watcher_modal {
                m.active_field = crate::app::WatcherField::Message;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_watcher_message_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Submit: add watcher for the selected session
            watcher::submit_watcher(app);
        }
        KeyCode::Enter => {
            if let Some(ref mut m) = app.watcher_modal {
                m.insert_newline();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut m) = app.watcher_modal {
                m.backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut m) = app.watcher_modal {
                m.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut m) = app.watcher_modal {
                m.cursor_right();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut m) = app.watcher_modal {
                m.cursor_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut m) = app.watcher_modal {
                m.cursor_down();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut m) = app.watcher_modal {
                m.insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_watcher_orig_message_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut m) = app.watcher_modal {
                if m.selected_message_idx > 0 {
                    m.selected_message_idx -= 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut m) = app.watcher_modal {
                if m.selected_message_idx + 1 < m.session_messages.len() {
                    m.selected_message_idx += 1;
                }
            }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            watcher::submit_watcher(app);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_watcher_timeout_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if let Some(ref mut m) = app.watcher_modal {
                m.timeout_input.push(c);
                if let Ok(val) = m.timeout_input.parse::<u64>() {
                    m.idle_timeout_secs = val;
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut m) = app.watcher_modal {
                m.timeout_input.pop();
                if m.timeout_input.is_empty() {
                    m.idle_timeout_secs = 0;
                } else if let Ok(val) = m.timeout_input.parse::<u64>() {
                    m.idle_timeout_secs = val;
                }
            }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            watcher::submit_watcher(app);
        }
        KeyCode::Enter => {
            watcher::submit_watcher(app);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_watcher_hang_message_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            watcher::submit_watcher(app);
        }
        KeyCode::Enter => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_insert_newline();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_cursor_right();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_cursor_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_cursor_down();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn handle_watcher_hang_timeout_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_timeout_input.push(c);
                if let Ok(val) = m.hang_timeout_input.parse::<u64>() {
                    m.hang_timeout_secs = val;
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut m) = app.watcher_modal {
                m.hang_timeout_input.pop();
                if m.hang_timeout_input.is_empty() {
                    m.hang_timeout_secs = 0;
                } else if let Ok(val) = m.hang_timeout_input.parse::<u64>() {
                    m.hang_timeout_secs = val;
                }
            }
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            watcher::submit_watcher(app);
        }
        KeyCode::Enter => {
            watcher::submit_watcher(app);
        }
        _ => {}
    }
    Ok(())
}
