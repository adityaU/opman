use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Clear, Widget};

use crate::app::App;
use crate::pty::CommandState;
use crate::theme::ansi_palette_from_theme;
use crate::ui::layout_manager::PanelId;
use crate::ui::sidebar::lerp_color;
use crate::ui::term_render;

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
                    term_render::render_screen(screen, content_area, buf, &palette, &self.app.theme);
                    render_url_underlines(buf, content_area, screen);
                }
                // Lock released — search/selection only touch the ratatui buffer.
                render_search_highlights(self.app, content_area, buf);

                if let Some(ref sel) = self.app.terminal_selection {
                    if sel.panel_id == PanelId::IntegratedTerminal {
                        let (sr, sc, er, ec) =
                            if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
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

                            for col in start_col..=end_col.min(content_area.width.saturating_sub(1))
                            {
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
                    term_render::render_screen(screen, content_area, buf, &palette, &self.app.theme);
                    render_url_underlines(buf, content_area, screen);
                }
                // Lock released — search/selection only touch the ratatui buffer.
                render_search_highlights(self.app, content_area, buf);

                if let Some(ref sel) = self.app.terminal_selection {
                    if sel.panel_id == PanelId::IntegratedTerminal {
                        let (sr, sc, er, ec) =
                            if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
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

                            for col in start_col..=end_col.min(content_area.width.saturating_sub(1))
                            {
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

fn render_search_highlights(app: &App, content_area: Rect, buf: &mut Buffer) {
    use ratatui::style::Color;

    if let Some(ref search) = app.terminal_search {
        if search.query.is_empty() {
            return;
        }

        let match_bg = Color::Rgb(100, 100, 40);
        let current_match_bg = Color::Rgb(200, 150, 0);
        let match_fg = Color::Rgb(255, 255, 255);

        for (i, &(row, col, len)) in search.matches.iter().enumerate() {
            let is_current = i == search.current_match;
            let bg = if is_current {
                current_match_bg
            } else {
                match_bg
            };

            for offset in 0..len {
                let x = content_area.x + (col + offset) as u16;
                let y = content_area.y + row as u16;
                if x < content_area.right() && y < content_area.bottom() {
                    let cell = buf.cell_mut((x, y)).expect("cell in bounds");
                    cell.set_bg(bg);
                    cell.set_fg(match_fg);
                    if is_current {
                        cell.set_style(
                            Style::default()
                                .fg(match_fg)
                                .bg(bg)
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }
}

fn render_search_bar(app: &App, content_area: Rect, buf: &mut Buffer) {
    use ratatui::style::Color;

    if let Some(ref search) = app.terminal_search {
        let bar_y = content_area.y;
        let bar_width = content_area.width.min(60);
        let bar_x = content_area.right().saturating_sub(bar_width);

        let bar_bg = Color::Rgb(50, 50, 50);
        let bar_fg = Color::Rgb(220, 220, 220);
        let input_fg = Color::Rgb(255, 255, 255);

        for x in bar_x..content_area.right() {
            if bar_y < content_area.bottom() {
                let cell = buf.cell_mut((x, bar_y)).expect("cell in bounds");
                cell.set_bg(bar_bg);
                cell.set_fg(bar_fg);
                cell.set_symbol(" ");
            }
        }

        let label = " Find: ";
        buf.set_string(bar_x, bar_y, label, Style::default().fg(bar_fg).bg(bar_bg));

        let query_x = bar_x + label.len() as u16;
        let max_query_len = (content_area
            .right()
            .saturating_sub(query_x)
            .saturating_sub(15)) as usize;
        let display_query = if search.query.len() > max_query_len {
            &search.query[search.query.len() - max_query_len..]
        } else {
            &search.query
        };
        buf.set_string(
            query_x,
            bar_y,
            display_query,
            Style::default()
                .fg(input_fg)
                .bg(bar_bg)
                .add_modifier(ratatui::style::Modifier::BOLD),
        );

        let cursor_x = query_x + display_query.len() as u16;
        if cursor_x < content_area.right().saturating_sub(15) {
            buf.set_string(
                cursor_x,
                bar_y,
                "▏",
                Style::default().fg(Color::Rgb(255, 255, 255)).bg(bar_bg),
            );
        }

        let match_info = if search.matches.is_empty() {
            if search.query.is_empty() {
                String::new()
            } else {
                " No matches ".to_string()
            }
        } else {
            format!(" {}/{} ", search.current_match + 1, search.matches.len())
        };

        if !match_info.is_empty() {
            let info_x = content_area.right().saturating_sub(match_info.len() as u16);
            buf.set_string(
                info_x,
                bar_y,
                &match_info,
                Style::default().fg(bar_fg).bg(bar_bg),
            );
        }
    }
}

fn render_url_underlines(buf: &mut Buffer, area: Rect, screen: &vt100::Screen) {
    let rows = area.height as usize;
    let cols = area.width as usize;
    for row in 0..rows {
        let line = screen.contents_between(row as u16, 0, row as u16, cols as u16);
        let mut search_from = 0;
        while search_from < line.len() {
            let remaining = &line[search_from..];
            let url_start = if let Some(pos) = remaining.find("https://") {
                Some(pos)
            } else if let Some(pos) = remaining.find("http://") {
                Some(pos)
            } else if let Some(pos) = remaining.find("ftp://") {
                Some(pos)
            } else {
                None
            };
            if let Some(start) = url_start {
                let abs_start = search_from + start;
                let url_bytes = remaining[start..].as_bytes();
                let mut end = 0;
                for &b in url_bytes {
                    if b == b' '
                        || b == b'\t'
                        || b == b'"'
                        || b == b'\''
                        || b == b'>'
                        || b == b'<'
                        || b == b'|'
                        || b == b'{'
                        || b == b'}'
                        || b == b'`'
                    {
                        break;
                    }
                    end += 1;
                }
                if end > 8 {
                    let url_style = Style::default()
                        .fg(Color::Rgb(100, 149, 237))
                        .add_modifier(Modifier::UNDERLINED);
                    for col in abs_start..abs_start + end {
                        if col < cols {
                            let x = area.x + col as u16;
                            let y = area.y + row as u16;
                            if x < area.x + area.width && y < area.y + area.height {
                                let cell = buf.cell_mut(ratatui::layout::Position::new(x, y));
                                if let Some(cell) = cell {
                                    cell.set_style(url_style);
                                }
                            }
                        }
                    }
                }
                search_from = abs_start + end;
            } else {
                break;
            }
        }
    }
}
