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

        if let Some(project) = self.app.active_project() {
            if let Some(pty) = project.active_pty() {
                {
                    let parser = match pty.parser.lock() {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    let palette = ansi_palette_from_theme(&self.app.theme);
                    let screen = parser.screen();
                    term_render::render_screen(screen, area, buf, &palette, &self.app.theme);
                }
                // Lock released — selection highlights only touch the ratatui buffer.

                // Render selection highlight
                if let Some(ref sel) = self.app.terminal_selection {
                    if sel.panel_id == crate::ui::layout_manager::PanelId::TerminalPane {
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

                return;
            }
        }

        let placeholder =
            Paragraph::new("No active terminal. Select a project and press Enter to connect.")
                .style(Style::default().fg(self.app.theme.text_muted));

        Widget::render(placeholder, area, buf);
    }
}
