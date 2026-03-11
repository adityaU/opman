use anyhow::Result;

use crate::app;
use crate::pty;
use crate::ui::layout_manager::PanelId;

/// Forward mouse events to a PTY.
/// Mouse mode active (vim, less, htop, opencode): forward as SGR bytes.
/// Mouse mode off (plain shell): scroll adjusts scrollback offset for viewing history.
pub(crate) fn forward_mouse_to_pty(
    pty: &mut pty::PtyInstance,
    event: &crossterm::event::MouseEvent,
    panel_x: u16,
    panel_y: u16,
    panel_id: PanelId,
    terminal_selection: &mut Option<app::TerminalSelection>,
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
                *terminal_selection = Some(app::TerminalSelection {
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
    if let Some(bytes) = crate::input::mouse_event_to_bytes(event, panel_x, panel_y) {
        let _ = pty.write(&bytes);
    }
}

/// Update terminal search matches based on current query.
/// Searches the full terminal buffer (scrollback + visible) for the query string.
pub(crate) fn update_terminal_search_matches(app: &mut app::App) {
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

/// Handle mouse events in the integrated terminal panel (tabs + content).
pub(crate) fn handle_integrated_terminal_mouse(
    app: &mut app::App,
    mouse_event: crossterm::event::MouseEvent,
) -> Result<()> {
    if let Some(project) = app.projects.get_mut(app.active_project) {
        if let Some(rect) = app.layout.panel_rect(PanelId::IntegratedTerminal) {
            let has_tab_bar = project
                .active_resources()
                .map(|r| r.shell_ptys.len())
                .unwrap_or(0)
                > 1;
            // Check if click is on the tab bar row
            if has_tab_bar && mouse_event.row == rect.y {
                if matches!(
                    mouse_event.kind,
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                ) {
                    // Calculate which tab was clicked based on x position
                    let click_x = mouse_event.column.saturating_sub(rect.x) as usize;
                    let mut x_offset = 0usize;
                    let mut clicked_tab = None;
                    if let Some(resources) = project.active_resources() {
                        for (i, pty) in resources.shell_ptys.iter().enumerate() {
                            let label = if pty.name.is_empty() {
                                format!(" Tab {} ", i + 1)
                            } else {
                                format!(" {} ", pty.name)
                            };
                            let label_len = label.len();
                            let cmd_state = pty.command_state.lock().unwrap().clone();
                            let dot_width = if cmd_state != crate::pty::CommandState::Idle {
                                1
                            } else {
                                0
                            };
                            if click_x >= x_offset && click_x < x_offset + label_len + dot_width {
                                clicked_tab = Some(i);
                                break;
                            }
                            x_offset += label_len + dot_width;
                        }
                    }
                    if let Some(tab) = clicked_tab {
                        if let Some(resources) = project.active_resources_mut() {
                            resources.active_shell_tab = tab;
                        }
                    }
                }
            } else if let Some(pty) = project.active_shell_pty_mut() {
                // Account for tab bar offset only when tab bar is visible
                let content_offset_y = if has_tab_bar { rect.y + 1 } else { rect.y };
                forward_mouse_to_pty(
                    pty,
                    &mouse_event,
                    rect.x,
                    content_offset_y,
                    PanelId::IntegratedTerminal,
                    &mut app.terminal_selection,
                    &mut app.toast_message,
                );
            }
        }
    }
    Ok(())
}
