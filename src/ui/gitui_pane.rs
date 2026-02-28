use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::app::App;
use crate::theme::ansi_palette_from_theme;
use crate::ui::term_render;

pub struct GituiPane<'a> {
    pub app: &'a App,
}

impl<'a> GituiPane<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl Widget for GituiPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let project = match self.app.active_project() {
            Some(p) => p,
            None => {
                render_no_gitui(buf, area, &self.app.theme);
                return;
            }
        };

        match &project.gitui_pty {
            Some(pty) => {
                {
                    let parser = match pty.parser.lock() {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let palette = ansi_palette_from_theme(&self.app.theme);
                    let screen = parser.screen();
                    term_render::render_screen(screen, area, buf, &palette, &self.app.theme);
                }

                if let Some(ref sel) = self.app.terminal_selection {
                    if sel.panel_id == crate::ui::layout_manager::PanelId::GitPanel {
                        let (sr, sc, er, ec) =
                            if (sel.start_row, sel.start_col) <= (sel.end_row, sel.end_col) {
                                (sel.start_row, sel.start_col, sel.end_row, sel.end_col)
                            } else {
                                (sel.end_row, sel.end_col, sel.start_row, sel.start_col)
                            };

                        for row in sr..=er.min(area.height.saturating_sub(1)) {
                            let start_col = if row == sr { sc } else { 0 };
                            let end_col = if row == er {
                                ec
                            } else {
                                area.width.saturating_sub(1)
                            };

                            for col in start_col..=end_col.min(area.width.saturating_sub(1)) {
                                let x = area.x + col;
                                let y = area.y + row;
                                if x < area.right() && y < area.bottom() {
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
            None => {
                render_no_gitui(buf, area, &self.app.theme);
            }
        }
    }
}

fn render_no_gitui(buf: &mut Buffer, area: Rect, theme: &crate::theme::ThemeColors) {
    let msg = "No gitui (install: cargo install gitui)";
    let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
    let y = area.y + area.height / 2;
    buf.set_string(x, y, msg, Style::default().fg(theme.text_muted));
}
