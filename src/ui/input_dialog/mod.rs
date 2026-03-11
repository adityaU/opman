mod completions;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::App;

pub(super) const MAX_VISIBLE_COMPLETIONS: usize = 8;

pub struct InputDialog<'a> {
    app: &'a App,
}

impl<'a> InputDialog<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.app.theme;
        let completion_rows = if self.app.completions_visible {
            std::cmp::min(self.app.completions.len(), MAX_VISIBLE_COMPLETIONS) as u16
        } else {
            0
        };

        let popup_width = 60u16.min(area.width.saturating_sub(2));
        let base_height: u16 = 6 + completion_rows;
        let popup_height = base_height.min(area.height.saturating_sub(4));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(theme.background_panel));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let content_area = Rect::new(
            inner.x + 2,
            inner.y + 1,
            inner.width.saturating_sub(4),
            inner.height.saturating_sub(1),
        );

        let title_line = Line::from(vec![
            Span::styled(
                "Add Project",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "enter directory path",
                Style::default().fg(theme.text_muted),
            ),
        ]);
        let esc_hint = Line::from(vec![Span::styled(
            "esc",
            Style::default().fg(theme.text_muted),
        )]);
        if content_area.height > 0 {
            let title_area = Rect::new(content_area.x, content_area.y, content_area.width, 1);
            buf.set_line(title_area.x, title_area.y, &title_line, title_area.width);
            let esc_w = 3u16;
            buf.set_line(
                title_area.right().saturating_sub(esc_w),
                title_area.y,
                &esc_hint,
                esc_w,
            );
        }

        let below_title = Rect::new(
            content_area.x,
            content_area.y + 1,
            content_area.width,
            content_area.height.saturating_sub(1),
        );

        let mut constraints = vec![Constraint::Length(1)];
        if completion_rows > 0 {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(completion_rows));
        }
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Min(0));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(below_title);

        self.render_input_line(chunks[0], buf);

        let hint_idx;
        if completion_rows > 0 {
            self.render_separator(chunks[1], buf);
            self.render_completions(chunks[2], buf);
            hint_idx = 3;
        } else {
            hint_idx = 1;
        }

        if hint_idx < chunks.len() {
            self.render_hint(chunks[hint_idx], buf);
        }
    }

    fn render_input_line(&self, area: Rect, buf: &mut Buffer) {
        let input_text = &self.app.input_buffer;
        let cursor_pos = self.app.input_cursor;

        let before_cursor = &input_text[..cursor_pos.min(input_text.len())];
        let cursor_char = input_text.get(cursor_pos..cursor_pos + 1).unwrap_or(" ");
        let after_cursor = if cursor_pos + 1 < input_text.len() {
            &input_text[cursor_pos + 1..]
        } else {
            ""
        };

        let input_line = Line::from(vec![
            Span::raw(before_cursor),
            Span::styled(
                cursor_char,
                Style::default()
                    .bg(self.app.theme.text)
                    .fg(self.app.theme.background)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(after_cursor),
        ]);

        Paragraph::new(input_line).render(area, buf);
    }

    fn render_separator(&self, area: Rect, buf: &mut Buffer) {
        let sep: String = "─".repeat(area.width as usize);
        let line = Line::from(Span::styled(
            sep,
            Style::default().fg(self.app.theme.border_subtle),
        ));
        Paragraph::new(line).render(area, buf);
    }

    fn render_hint(&self, area: Rect, buf: &mut Buffer) {
        let hint = if self.app.completions_visible {
            Line::from(vec![
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(self.app.theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " complete  ",
                    Style::default().fg(self.app.theme.text_muted),
                ),
                Span::styled(
                    "↑↓",
                    Style::default()
                        .fg(self.app.theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" select  ", Style::default().fg(self.app.theme.text_muted)),
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(self.app.theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm  ", Style::default().fg(self.app.theme.text_muted)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(self.app.theme.warning)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" close", Style::default().fg(self.app.theme.text_muted)),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(self.app.theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " complete  ",
                    Style::default().fg(self.app.theme.text_muted),
                ),
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(self.app.theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" confirm  ", Style::default().fg(self.app.theme.text_muted)),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(self.app.theme.warning)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::default().fg(self.app.theme.text_muted)),
            ])
        };

        Paragraph::new(hint).render(area, buf);
    }
}
