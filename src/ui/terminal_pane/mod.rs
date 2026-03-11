mod watcher_overlay;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Widget};

use crate::app::App;
use crate::theme::ansi_palette_from_theme;
use crate::ui::term_render;

pub struct TerminalPane<'a> {
    app: &'a App,
}

impl<'a> TerminalPane<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl<'a> Widget for TerminalPane<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Determine if there's an active watcher for the current session.
        let watcher_info = self.watcher_overlay_info();
        let has_overlay = watcher_info.is_some();

        // Split area: 1 row for overlay at top if watcher active, rest for PTY.
        let (overlay_area, pty_area) = if has_overlay && area.height > 2 {
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
                Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: area.height - 1,
                },
            )
        } else {
            (Rect::default(), area)
        };

        if let Some(project) = self.app.active_project() {
            if let Some(pty) = project.active_pty() {
                {
                    let parser = match pty.parser.lock() {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let palette = ansi_palette_from_theme(&self.app.theme);
                    let screen = parser.screen();
                    term_render::render_screen(screen, pty_area, buf, &palette, &self.app.theme);
                }

                // Render selection highlight
                if let Some(ref sel) = self.app.terminal_selection {
                    if sel.panel_id == crate::ui::layout_manager::PanelId::TerminalPane {
                        let (sr, sc, er, ec) =
                            if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
                                (sel.start_row, sel.start_col, sel.end_row, sel.end_col)
                            } else {
                                (sel.end_row, sel.end_col, sel.start_row, sel.start_col)
                            };

                        for row in sr..=er.min(pty_area.height.saturating_sub(1)) {
                            let start_col = if row == sr { sc } else { 0 };
                            let end_col = if row == er {
                                ec
                            } else {
                                pty_area.width.saturating_sub(1)
                            };

                            for col in start_col..=end_col.min(pty_area.width.saturating_sub(1)) {
                                let x = pty_area.x + col;
                                let y = pty_area.y + row;
                                if x < pty_area.right() && y < pty_area.bottom() {
                                    let cell = buf.cell_mut((x, y)).expect("cell in bounds");
                                    let fg = cell.fg;
                                    cell.set_fg(cell.bg);
                                    cell.set_bg(fg);
                                }
                            }
                        }
                    }
                }

                // Render watcher overlay bar if active.
                if let Some(info) = watcher_info {
                    self.render_watcher_overlay(overlay_area, buf, &info);
                }

                return;
            }
        }

        let placeholder =
            Paragraph::new("No active terminal. Select a project and press Enter to connect.")
                .style(Style::default().fg(self.app.theme.text_muted));

        Widget::render(placeholder, area, buf);
    }
}

/// Info needed to render the watcher overlay.
pub(crate) struct WatcherOverlayInfo {
    /// Session is currently busy (running).
    pub(crate) is_busy: bool,
    /// Children are active (suppressing the watcher).
    pub(crate) children_active: bool,
    /// Configured idle timeout in seconds.
    pub(crate) timeout_secs: u64,
    /// Seconds elapsed since session went idle (None if busy or children active).
    pub(crate) idle_elapsed_secs: Option<u64>,
    /// Seconds since last activity across all signals (None if not busy or no watcher).
    pub(crate) hang_silent_secs: Option<u64>,
    /// Configured hang detection timeout in seconds.
    pub(crate) hang_timeout_secs: u64,
}

impl<'a> TerminalPane<'a> {
    /// Compute watcher overlay info for the current session, if a watcher is active.
    fn watcher_overlay_info(&self) -> Option<WatcherOverlayInfo> {
        let project = self.app.active_project()?;
        let session_id = project.active_session.as_ref()?;
        let watcher = self.app.session_watchers.get(session_id)?;

        let is_busy = self.app.active_sessions.contains(session_id);
        let children_active = self
            .app
            .session_children
            .get(session_id)
            .map(|children| {
                children
                    .iter()
                    .any(|cid| self.app.active_sessions.contains(cid))
            })
            .unwrap_or(false);

        let idle_elapsed_secs = if !is_busy && !children_active {
            self.app
                .watcher_idle_since
                .get(session_id)
                .map(|since| since.elapsed().as_secs())
        } else {
            None
        };

        let hang_silent_secs = if is_busy && !children_active {
            self.app.hang_silent_secs(session_id)
        } else {
            None
        };

        Some(WatcherOverlayInfo {
            is_busy,
            children_active,
            timeout_secs: watcher.idle_timeout_secs,
            idle_elapsed_secs,
            hang_silent_secs,
            hang_timeout_secs: watcher.hang_timeout_secs,
        })
    }
}
