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

        // Text area with word wrapping
        let text_y = inner.y + 3;
        let text_height = inner.height.saturating_sub(5) as usize; // title + sep + hint
        let cw = content_width as usize;

        // Build visual rows from logical lines, wrapping at content_width.
        // Each entry is (logical_line_idx, byte_start, byte_end).
        let mut vrows: Vec<(usize, usize, usize)> = Vec::new();
        let mut cursor_vrow: usize = 0; // visual row of cursor
        let mut _cursor_vcol: usize = 0; // visual column of cursor

        for (li, line_text) in state.lines.iter().enumerate() {
            if cw == 0 {
                vrows.push((li, 0, line_text.len()));
                continue;
            }
            if line_text.is_empty() {
                if li == state.cursor_row {
                    cursor_vrow = vrows.len();
                    _cursor_vcol = 0;
                }
                vrows.push((li, 0, 0));
                continue;
            }
            let mut pos = 0;
            while pos < line_text.len() {
                let chunk_start = pos;
                let mut col = 0;
                let mut end = pos;
                for ch in line_text[pos..].chars() {
                    if col + 1 > cw {
                        break;
                    }
                    col += 1;
                    end += ch.len_utf8();
                }
                if end == pos {
                    let ch = line_text[pos..].chars().next().unwrap();
                    end += ch.len_utf8();
                }
                if li == state.cursor_row
                    && state.cursor_col >= chunk_start
                    && (state.cursor_col < end
                        || (state.cursor_col == end && end == line_text.len()))
                {
                    cursor_vrow = vrows.len();
                    _cursor_vcol = line_text[chunk_start..state.cursor_col.min(end)]
                        .chars()
                        .count();
                }
                vrows.push((li, chunk_start, end));
                pos = end;
            }
        }

        // Scroll so cursor is visible
        let visible_start = if cursor_vrow >= text_height {
            cursor_vrow - text_height + 1
        } else {
            0
        };

        for vi in 0..text_height {
            let vrow_idx = visible_start + vi;
            let row_y = text_y + vi as u16;
            if row_y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }
            if vrow_idx >= vrows.len() {
                break;
            }

            let (li, byte_start, byte_end) = vrows[vrow_idx];
            let chunk = &state.lines[li][byte_start..byte_end];
            let is_cursor_vrow = vrow_idx == cursor_vrow;

            if is_cursor_vrow {
                let cursor_byte = state.cursor_col.min(byte_end).saturating_sub(byte_start);
                let before = &chunk[..cursor_byte.min(chunk.len())];
                let (cursor_ch, cursor_ch_len) = if cursor_byte < chunk.len() {
                    let ch = chunk[cursor_byte..].chars().next().unwrap();
                    (
                        &chunk[cursor_byte..cursor_byte + ch.len_utf8()],
                        ch.len_utf8(),
                    )
                } else {
                    (" ", 0)
                };
                let after = if cursor_ch_len > 0 && cursor_byte + cursor_ch_len <= chunk.len() {
                    &chunk[cursor_byte + cursor_ch_len..]
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
                buf.set_string(content_x, row_y, chunk, Style::default().fg(theme.text));
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
