use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

use crate::app::App;

pub struct ContextInput<'a> {
    app: &'a App,
}

impl<'a> ContextInput<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.app.theme;
        let state = match &self.app.context_input {
            Some(s) => s,
            None => return,
        };

        let popup_width = (area.width * 60 / 100)
            .max(40)
            .min(area.width.saturating_sub(2));
        let popup_height = (area.height * 50 / 100)
            .max(10)
            .min(area.height.saturating_sub(4));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block =
            ratatui::widgets::Block::default().style(Style::default().bg(theme.background_panel));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let content_x = inner.x + 2;
        let content_width = inner.width.saturating_sub(4);

        // Title bar
        if inner.height < 3 {
            return;
        }
        let title_line = Line::from(vec![
            Span::styled(
                "Context Input",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "insert context for OpenCode",
                Style::default().fg(theme.text_muted),
            ),
        ]);
        let esc_hint = Span::styled("esc", Style::default().fg(theme.text_muted));
        buf.set_line(content_x, inner.y + 1, &title_line, content_width);
        let esc_w = 3u16;
        buf.set_string(
            content_x + content_width.saturating_sub(esc_w),
            inner.y + 1,
            esc_hint.content.as_ref(),
            Style::default().fg(theme.text_muted),
        );

        // Separator
        let sep: String = "─".repeat(content_width as usize);
        buf.set_string(
            content_x,
            inner.y + 2,
            &sep,
            Style::default().fg(theme.border_subtle),
        );

        // Text area
        let text_y = inner.y + 3;
        let text_height = inner.height.saturating_sub(5); // title + sep + hint

        let visible_start = if state.cursor_row >= text_height as usize {
            state.cursor_row - text_height as usize + 1
        } else {
            0
        };

        for i in 0..text_height as usize {
            let line_idx = visible_start + i;
            let row_y = text_y + i as u16;
            if row_y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }

            if line_idx < state.lines.len() {
                let line_text = &state.lines[line_idx];
                let is_cursor_line = line_idx == state.cursor_row;

                if is_cursor_line {
                    let col = state.cursor_col.min(line_text.len());
                    let before = &line_text[..col];
                    let cursor_ch = line_text.get(col..col + 1).unwrap_or(" ");
                    let after = if col + 1 < line_text.len() {
                        &line_text[col + 1..]
                    } else {
                        ""
                    };

                    let spans = vec![
                        Span::styled(before, Style::default().fg(theme.text)),
                        Span::styled(
                            cursor_ch,
                            Style::default()
                                .bg(theme.text)
                                .fg(theme.background)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(after, Style::default().fg(theme.text)),
                    ];
                    let line = Line::from(spans);
                    Paragraph::new(line).render(Rect::new(content_x, row_y, content_width, 1), buf);
                } else {
                    buf.set_string(content_x, row_y, line_text, Style::default().fg(theme.text));
                }
            }
        }

        // Hint bar at bottom
        let hint_y = inner.y + inner.height.saturating_sub(1);
        let hint = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" newline  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "Ctrl+D",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" submit  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(theme.text_muted)),
        ]);
        Paragraph::new(hint).render(Rect::new(content_x, hint_y, content_width, 1), buf);
    }
}
