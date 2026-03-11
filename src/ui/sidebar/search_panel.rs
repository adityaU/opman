use std::cmp::min;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Clear, Widget};

use crate::app::App;

const MAX_VISIBLE_SEARCH_RESULTS: usize = 20;

pub struct SessionSearchPanel<'a> {
    app: &'a App,
}

impl<'a> SessionSearchPanel<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(&self, area: Rect, buf: &mut Buffer) {
        let popup_width = ((area.width as f32) * 0.6) as u16;
        let popup_height = ((area.height as f32) * 0.5) as u16;
        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(self.app.theme.background_panel));
        Widget::render(block, popup_area, buf);

        let title_area = Rect::new(
            popup_area.x + 1,
            popup_area.y,
            popup_area.width.saturating_sub(2),
            1,
        );
        buf.set_string(
            title_area.x,
            title_area.y,
            "Search Sessions",
            Style::default()
                .fg(self.app.theme.text)
                .add_modifier(Modifier::BOLD),
        );

        let input_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + 1,
            popup_area.width.saturating_sub(2),
            1,
        );
        let input_text = format!("> {}", self.app.session_search_buffer);
        buf.set_string(
            input_area.x,
            input_area.y,
            &input_text,
            Style::default().fg(self.app.theme.text),
        );

        let cursor_x = input_area.x + 2 + self.app.session_search_cursor as u16;
        if cursor_x < input_area.x + input_area.width {
            buf.set_string(
                cursor_x,
                input_area.y,
                " ",
                Style::default()
                    .bg(self.app.theme.text)
                    .fg(self.app.theme.background),
            );
        }

        let sep_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + 2,
            popup_area.width.saturating_sub(2),
            1,
        );
        let sep = "\u{2500}".repeat(sep_area.width as usize);
        buf.set_string(
            sep_area.x,
            sep_area.y,
            &sep,
            Style::default().fg(self.app.theme.border_subtle),
        );

        let list_y_start = popup_area.y + 3;
        let max_visible = min(
            (popup_area.height as usize).saturating_sub(4),
            MAX_VISIBLE_SEARCH_RESULTS,
        );

        let selected = self.app.session_search_selected;
        let total = self.app.session_search_results.len();
        let scroll_offset = if selected >= max_visible {
            selected - max_visible + 1
        } else {
            0
        };
        let end = min(scroll_offset + max_visible, total);

        for (i, idx) in (scroll_offset..end).enumerate() {
            let row_y = list_y_start + i as u16;
            if row_y >= popup_area.y + popup_area.height.saturating_sub(1) {
                break;
            }

            let session = &self.app.session_search_results[idx];
            let is_selected = idx == selected;

            let style = if is_selected {
                Style::default()
                    .bg(self.app.theme.primary)
                    .fg(self.app.theme.background)
            } else {
                Style::default().fg(self.app.theme.text)
            };

            let title = if session.title.is_empty() {
                &session.id
            } else {
                &session.title
            };
            let max_title_len = (popup_area.width.saturating_sub(4)) as usize;
            let truncated = if title.len() > max_title_len {
                &title[..max_title_len.saturating_sub(3)]
            } else {
                title
            };

            let display = format!("  {}", truncated);
            buf.set_string(popup_area.x + 1, row_y, &display, style);

            if is_selected {
                let remaining = (popup_area.width as usize).saturating_sub(display.len() + 1);
                if remaining > 0 {
                    buf.set_string(
                        popup_area.x + 1 + display.len() as u16,
                        row_y,
                        &" ".repeat(remaining),
                        style,
                    );
                }
            }
        }

        if total > max_visible {
            let indicator = format!(" [{}/{}]", selected + 1, total);
            let indicator_x =
                popup_area.x + popup_area.width.saturating_sub(indicator.len() as u16 + 1);
            let indicator_y = popup_area.y + popup_area.height.saturating_sub(2);
            buf.set_string(
                indicator_x,
                indicator_y,
                &indicator,
                Style::default().fg(self.app.theme.text_muted),
            );
        }

        let hint_y = popup_area.y + popup_area.height.saturating_sub(1);
        buf.set_string(
            popup_area.x + 1,
            hint_y,
            "\u{2191}\u{2193} navigate  Enter select  Esc cancel",
            Style::default().fg(self.app.theme.text_muted),
        );
    }
}
