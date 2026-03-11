use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::App;
use crate::background_tasks::{spawn_activate_project, spawn_single_session_fetch};
use crate::input;
use crate::mouse_handler::update_terminal_search_matches;
use crate::ui::layout_manager::PanelId;

/// Handle a key event inside the event loop.
pub(crate) fn handle_key_in_loop(app: &mut App, key: crossterm::event::KeyEvent) -> Result<()> {
    let mut key_intercepted = false;
    // Terminal search input handling — intercept keys before normal handler
    if app.terminal_search.is_some() {
        let handled = match key.code {
            KeyCode::Esc => {
                app.terminal_search = None;
                true
            }
            KeyCode::Enter => {
                // Next match
                if let Some(ref mut search) = app.terminal_search {
                    if !search.matches.is_empty() {
                        search.current_match = (search.current_match + 1) % search.matches.len();
                        // Scroll to match
                        let (match_row, _, _) = search.matches[search.current_match];
                        if let Some(project) = app.projects.get_mut(app.active_project) {
                            if let Some(pty) = project.active_shell_pty_mut() {
                                // Count lines and set scrollback in one lock
                                // to avoid clone + double-lock.
                                if let Ok(mut p) = pty.parser.lock() {
                                    let rows = p.screen().size().0 as usize;
                                    let total = p.screen().contents().lines().count();
                                    if match_row + rows < total {
                                        pty.scroll_offset = total - match_row - rows;
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
                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
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
                } else if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
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
            key_intercepted = true;
        }
    }

    if !key_intercepted {
        // Shift+PgUp/PgDn for terminal keyboard scrollback
        {
            let is_terminal_focused = matches!(app.layout.focused, PanelId::IntegratedTerminal);
            if is_terminal_focused {
                match (key.code, key.modifiers.contains(KeyModifiers::SHIFT)) {
                    (KeyCode::PageUp, true) => {
                        if let Some(project) = app.projects.get_mut(app.active_project) {
                            if let Some(pty) = project.active_shell_pty_mut() {
                                pty.scroll_offset =
                                    pty.scroll_offset.saturating_add(pty.rows as usize / 2);
                                if let Ok(mut p) = pty.parser.lock() {
                                    p.set_scrollback(pty.scroll_offset);
                                    pty.scroll_offset = p.screen().scrollback();
                                }
                            }
                        }
                        key_intercepted = true;
                    }
                    (KeyCode::PageDown, true) => {
                        if let Some(project) = app.projects.get_mut(app.active_project) {
                            if let Some(pty) = project.active_shell_pty_mut() {
                                pty.scroll_offset =
                                    pty.scroll_offset.saturating_sub(pty.rows as usize / 2);
                                if let Ok(mut p) = pty.parser.lock() {
                                    p.set_scrollback(pty.scroll_offset);
                                    pty.scroll_offset = p.screen().scrollback();
                                }
                            }
                        }
                        key_intercepted = true;
                    }
                    _ => {}
                }
            }
        }
    } // !key_intercepted (search + scroll)

    if !key_intercepted {
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
                let content_area = ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
                app.layout.compute_rects(content_area);
                let (inner_cols, inner_rows) = app
                    .layout
                    .panel_rect(PanelId::TerminalPane)
                    .map(|r| (r.width, r.height))
                    .unwrap_or((cols.saturating_sub(32), rows.saturating_sub(2)));
                let path = app.projects[new_idx].path.clone();
                let theme_envs = app.theme.pty_env_vars();
                spawn_activate_project(
                    &app.bg_tx, new_idx, path, inner_rows, inner_cols, theme_envs,
                );
            }
        }
    } // !key_intercepted (normal key handling)

    Ok(())
}
