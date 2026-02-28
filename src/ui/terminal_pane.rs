use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::app::App;
use crate::theme::ansi_palette_from_theme;
use crate::ui::sidebar::lerp_color;
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
struct WatcherOverlayInfo {
    /// Session is currently busy (running).
    is_busy: bool,
    /// Children are active (suppressing the watcher).
    children_active: bool,
    /// Configured idle timeout in seconds.
    timeout_secs: u64,
    /// Seconds elapsed since session went idle (None if busy or children active).
    idle_elapsed_secs: Option<u64>,
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

        Some(WatcherOverlayInfo {
            is_busy,
            children_active,
            timeout_secs: watcher.idle_timeout_secs,
            idle_elapsed_secs,
        })
    }

    /// Render the watcher status bar overlay.
    fn render_watcher_overlay(&self, area: Rect, buf: &mut Buffer, info: &WatcherOverlayInfo) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let theme = &self.app.theme;

        // Fill background
        let bg_style = Style::default()
            .bg(theme.background_panel)
            .fg(theme.text_muted);
        for x in area.x..area.right() {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_style(bg_style);
                cell.set_char(' ');
            }
        }

        let mut spans = Vec::new();

        // Watcher label
        spans.push(Span::styled(
            " watcher ",
            Style::default()
                .fg(theme.background)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));

        if info.is_busy || info.children_active {
            // Running state — pulsing dot
            let dot_color = lerp_color(theme.background_panel, theme.warning, self.app.pulse_phase);
            spans.push(Span::styled(
                "\u{25CF}",
                Style::default().fg(dot_color).bg(theme.background_panel),
            ));

            if info.children_active {
                spans.push(Span::styled(
                    " running ",
                    Style::default()
                        .fg(theme.warning)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    "(subagent active)",
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
            } else {
                spans.push(Span::styled(
                    " running",
                    Style::default()
                        .fg(theme.warning)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else if let Some(elapsed) = info.idle_elapsed_secs {
            // Idle state with countdown
            let remaining = info.timeout_secs.saturating_sub(elapsed);

            // Pulsing dot (slower, calmer pulse for idle)
            let dot_color = lerp_color(theme.background_panel, theme.success, self.app.pulse_phase);
            spans.push(Span::styled(
                "\u{25CF}",
                Style::default().fg(dot_color).bg(theme.background_panel),
            ));
            spans.push(Span::styled(
                " idle ",
                Style::default()
                    .fg(theme.success)
                    .bg(theme.background_panel)
                    .add_modifier(Modifier::BOLD),
            ));

            if remaining > 0 {
                spans.push(Span::styled(
                    format!("{}s", elapsed),
                    Style::default()
                        .fg(theme.text)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" / {}s", info.timeout_secs),
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
                spans.push(Span::styled(
                    " \u{2014} ",
                    Style::default()
                        .fg(theme.text_muted)
                        .bg(theme.background_panel),
                ));
                spans.push(Span::styled(
                    format!("continuing in {}s", remaining),
                    Style::default().fg(theme.accent).bg(theme.background_panel),
                ));
            } else {
                spans.push(Span::styled(
                    "sending continuation...",
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.background_panel)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            // Watcher configured but no idle data yet (just set up)
            spans.push(Span::styled(
                "\u{25CB} waiting",
                Style::default()
                    .fg(theme.text_muted)
                    .bg(theme.background_panel),
            ));
        }

        // Right-aligned timeout info
        let timeout_label = format!(" {}s timeout ", info.timeout_secs);
        let spans_width: usize = spans.iter().map(|s| s.content.len()).sum();
        let right_pad = (area.width as usize).saturating_sub(spans_width + timeout_label.len());
        if right_pad > 0 {
            spans.push(Span::styled(
                " ".repeat(right_pad),
                Style::default().bg(theme.background_panel),
            ));
        }
        spans.push(Span::styled(
            timeout_label,
            Style::default()
                .fg(theme.text_muted)
                .bg(theme.background_panel),
        ));

        let line = Line::from(spans);
        let para = Paragraph::new(line);
        Widget::render(para, area, buf);
    }
}
