mod config_panel;
mod editor;
mod hang_section;
mod hint_bar;
mod session_list;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Widget};

use crate::app::App;

use config_panel::render_config_panel;
use hint_bar::render_hint_bar;
use session_list::render_session_list;

pub struct WatcherModal<'a> {
    app: &'a App,
}

impl<'a> WatcherModal<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.app.theme;
        let state = match &self.app.watcher_modal {
            Some(s) => s,
            None => return,
        };

        // 70% width, 70% height, centered
        let popup_width = (area.width * 70 / 100)
            .max(60)
            .min(area.width.saturating_sub(2));
        let popup_height = (area.height * 70 / 100)
            .max(14)
            .min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(theme.background_panel));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        if inner.height < 6 || inner.width < 20 {
            return;
        }

        let content_x = inner.x + 1;
        let content_width = inner.width.saturating_sub(2);

        // Title bar
        let title_line = Line::from(vec![
            Span::styled(
                " Session Watcher ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " monitor idle sessions",
                Style::default().fg(theme.text_muted),
            ),
        ]);
        let esc_hint = "esc";
        buf.set_line(content_x, inner.y + 1, &title_line, content_width);
        buf.set_string(
            content_x + content_width.saturating_sub(esc_hint.len() as u16),
            inner.y + 1,
            esc_hint,
            Style::default().fg(theme.text_muted),
        );

        // Separator below title
        let sep: String = "─".repeat(content_width as usize);
        buf.set_string(
            content_x,
            inner.y + 2,
            &sep,
            Style::default().fg(theme.border_subtle),
        );

        // Two-panel layout: left 30%, right 70%
        let body_y = inner.y + 3;
        let body_height = inner.height.saturating_sub(5); // title(2) + sep(1) + hint(1) + pad(1)
        let left_width = (content_width * 30 / 100).max(15);
        let right_width = content_width.saturating_sub(left_width + 1); // 1 for separator

        let left_area = Rect::new(content_x, body_y, left_width, body_height);
        let sep_x = content_x + left_width;
        let right_area = Rect::new(sep_x + 1, body_y, right_width, body_height);

        // Vertical separator
        for row in body_y..body_y + body_height {
            buf.set_string(sep_x, row, "│", Style::default().fg(theme.border_subtle));
        }

        // Render left panel (session list)
        render_session_list(buf, left_area, state, theme);

        // Render right panel (watcher config)
        render_config_panel(buf, right_area, state, theme);

        // Hint bar at bottom
        let hint_y = inner.y + inner.height.saturating_sub(1);
        render_hint_bar(buf, content_x, hint_y, content_width, state, theme);
    }
}
