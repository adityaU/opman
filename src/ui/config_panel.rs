use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, Widget};

use crate::theme::ThemeColors;

pub struct ConfigPanel<'a> {
    theme: &'a ThemeColors,
    selected: usize,
    settings: Vec<(&'static str, bool)>,
}

impl<'a> ConfigPanel<'a> {
    pub fn new(theme: &'a ThemeColors, selected: usize, follow_edits_in_neovim: bool) -> Self {
        let settings = vec![("Follow edits in neovim", follow_edits_in_neovim)];
        Self {
            theme,
            selected,
            settings,
        }
    }

    pub fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        let item_count = self.settings.len() as u16;

        let popup_width = 60u16.min(area.width.saturating_sub(2));
        let popup_height = (item_count + 5).min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(self.theme.background_panel));
        let panel_inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let inner_x = panel_inner.x + 2;
        let inner_width = panel_inner.width.saturating_sub(4);

        // Title row: "Settings" left, "esc" right
        let title_y = panel_inner.y;
        buf.set_string(
            inner_x,
            title_y,
            "Settings",
            Style::default()
                .fg(self.theme.text)
                .add_modifier(Modifier::BOLD),
        );
        let esc_hint = "esc";
        let esc_x = inner_x + inner_width.saturating_sub(esc_hint.len() as u16);
        buf.set_string(
            esc_x,
            title_y,
            esc_hint,
            Style::default().fg(self.theme.text_muted),
        );

        // Separator
        let sep_y = title_y + 1;
        let sep = "─".repeat(inner_width as usize);
        buf.set_string(
            inner_x,
            sep_y,
            &sep,
            Style::default().fg(self.theme.border_subtle),
        );

        // Settings items
        let mut cy = sep_y + 1;
        let max_y = popup_area.y + popup_area.height - 1;

        for (i, (label, enabled)) in self.settings.iter().enumerate() {
            if cy >= max_y {
                break;
            }

            let is_selected = i == self.selected;
            let checkbox = if *enabled { "[✓] " } else { "[ ] " };

            let checkbox_style = if is_selected {
                Style::default()
                    .fg(self.theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_muted)
            };

            let label_style = if is_selected {
                Style::default()
                    .fg(self.theme.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.text_muted)
            };

            if is_selected {
                let highlight_style = Style::default().bg(self.theme.background_element);
                for col in inner_x..(inner_x + inner_width) {
                    buf.set_string(col, cy, " ", highlight_style);
                }
            }

            buf.set_string(inner_x, cy, checkbox, checkbox_style);
            buf.set_string(inner_x + 4, cy, label, label_style);

            cy += 1;
        }

        // Hint at bottom
        let hint = "↑↓ navigate · Enter toggle · Esc close";
        let hint_y = popup_area.y + popup_area.height - 1;
        if hint_y > cy {
            buf.set_string(
                inner_x,
                hint_y,
                hint,
                Style::default().fg(self.theme.text_muted),
            );
        }
    }
}
