mod search;
mod url;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Clear, Widget};

use crate::app::App;
use crate::theme::ansi_palette_from_theme;
use crate::ui::layout_manager::PanelId;
use crate::ui::sidebar::lerp_color;
use crate::ui::term_render;

use self::search::{render_search_bar, render_search_highlights};
use self::url::render_url_underlines;

/// Widget that renders the integrated shell terminal panel.
///
/// Displays the per-project shell PTY at the bottom of the screen.
/// Supports normal (1/3), expanded (2/3), and floating (fullscreen) modes.
pub struct IntegratedTerminal<'a> {
    app: &'a App,
}

impl<'a> IntegratedTerminal<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    fn render_tab_bar(&self, project: &crate::app::Project, area: Rect, buf: &mut Buffer) {
        use crate::pty::CommandState;
        use ratatui::style::Color;

        let theme = &self.app.theme;
        let bg_style = Style::default().bg(theme.background).fg(theme.text_muted);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(bg_style);
        }

        let resources = match project.active_resources() {
            Some(r) => r,
            None => return,
        };
        let mut x_offset = area.x;
        for (i, pty) in resources.shell_ptys.iter().enumerate() {
            if x_offset >= area.x + area.width {
                break;
            }

            let label = if pty.name.is_empty() {
                format!(" Tab {} ", i + 1)
            } else {
                format!(" {} ", pty.name)
            };
            let is_active = i == resources.active_shell_tab;

            let style = if is_active {
                Style::default()
                    .bg(theme.background)
                    .fg(theme.accent)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().bg(theme.background).fg(theme.text_muted)
            };

            buf.set_string(x_offset, area.y, &label, style);
            x_offset += label.len() as u16;

            let cmd_state = pty.command_state.lock().unwrap().clone();
            let dot_style = match cmd_state {
                CommandState::Running => {
                    let dot_color =
                        lerp_color(theme.background, theme.accent, self.app.pulse_phase);
                    Style::default().fg(dot_color).bg(theme.background)
                }
                CommandState::Success => Style::default()
                    .fg(Color::Rgb(80, 200, 80))
                    .bg(theme.background),
                CommandState::Failure => Style::default()
                    .fg(Color::Rgb(220, 60, 60))
                    .bg(theme.background),
                CommandState::Idle => Style::default().fg(theme.background).bg(theme.background),
            };
            if cmd_state != CommandState::Idle && x_offset < area.x + area.width {
                buf.set_string(x_offset, area.y, "●", dot_style);
                x_offset += 1;
            }
        }
    }

    /// Render the terminal in floating fullscreen overlay mode.
    #[allow(dead_code)]
    pub fn render_floating(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let base_style = Style::default()
            .fg(self.app.theme.text)
            .bg(self.app.theme.background);

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_style(base_style);
            }
        }

        if let Some(project) = self.app.active_project() {
            if let Some(shell_pty) = project.active_shell_pty() {
                let tab_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                };
                let content_area = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: area.height.saturating_sub(1),
                };
                self.render_tab_bar(project, tab_area, buf);

                {
                    let parser = match shell_pty.parser.lock() {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let palette = ansi_palette_from_theme(&self.app.theme);
                    let screen = parser.screen();
                    term_render::render_screen(
                        screen,
                        content_area,
                        buf,
                        &palette,
                        &self.app.theme,
                    );
                    render_url_underlines(buf, content_area, screen);
                }
                // Lock released — search/selection only touch the ratatui buffer.
                render_search_highlights(self.app, content_area, buf);
                render_selection(self.app, content_area, buf);
                render_search_bar(self.app, content_area, buf);
            } else {
                render_no_shell(area, buf, &self.app.theme);
            }
        } else {
            render_no_shell(area, buf, &self.app.theme);
        }
    }
}

impl<'a> Widget for IntegratedTerminal<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.app.theme;

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_style(Style::default().bg(theme.background));
            }
        }

        if let Some(project) = self.app.active_project() {
            if let Some(shell_pty) = project.active_shell_pty() {
                let tab_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                };
                let content_area = Rect {
                    x: area.x,
                    y: area.y + 1,
                    width: area.width,
                    height: area.height.saturating_sub(1),
                };
                self.render_tab_bar(project, tab_area, buf);

                {
                    let parser = match shell_pty.parser.lock() {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let palette = ansi_palette_from_theme(&self.app.theme);
                    let screen = parser.screen();
                    term_render::render_screen(
                        screen,
                        content_area,
                        buf,
                        &palette,
                        &self.app.theme,
                    );
                    render_url_underlines(buf, content_area, screen);
                }
                // Lock released — search/selection only touch the ratatui buffer.
                render_search_highlights(self.app, content_area, buf);
                render_selection(self.app, content_area, buf);
                render_search_bar(self.app, content_area, buf);
            } else {
                render_no_shell(area, buf, theme);
            }
        } else {
            render_no_shell(area, buf, theme);
        }
    }
}

fn render_no_shell(area: Rect, buf: &mut Buffer, theme: &crate::theme::ThemeColors) {
    let msg = "No shell (press ^T to initialize)";
    let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, msg, Style::default().fg(theme.text_muted));
}

fn render_selection(app: &App, content_area: Rect, buf: &mut Buffer) {
    if let Some(ref sel) = app.terminal_selection {
        if sel.panel_id == PanelId::IntegratedTerminal {
            let (sr, sc, er, ec) = if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
                (sel.start_row, sel.start_col, sel.end_row, sel.end_col)
            } else {
                (sel.end_row, sel.end_col, sel.start_row, sel.start_col)
            };

            for row in sr..=er.min(content_area.height.saturating_sub(1)) {
                let start_col = if row == sr { sc } else { 0 };
                let end_col = if row == er {
                    ec
                } else {
                    content_area.width.saturating_sub(1)
                };

                for col in start_col..=end_col.min(content_area.width.saturating_sub(1)) {
                    let x = content_area.x + col;
                    let y = content_area.y + row;
                    if x < content_area.right() && y < content_area.bottom() {
                        let cell = buf.cell_mut((x, y)).expect("cell in bounds");
                        let fg = cell.fg;
                        cell.set_fg(cell.bg);
                        cell.set_bg(fg);
                    }
                }
            }
        }
    }
}
