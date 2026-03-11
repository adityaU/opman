use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme::ThemeColors;

pub(super) fn render_multiline_editor(
    buf: &mut Buffer,
    cx: u16,
    y: u16,
    cw: usize,
    height: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    render_multiline_editor_generic(
        buf,
        cx,
        y,
        cw,
        height,
        &state.message_lines,
        state.message_cursor_row,
        state.message_cursor_col,
        theme,
    );
}

pub(super) fn render_hang_message_editor(
    buf: &mut Buffer,
    cx: u16,
    y: u16,
    cw: usize,
    height: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    render_multiline_editor_generic(
        buf,
        cx,
        y,
        cw,
        height,
        &state.hang_message_lines,
        state.hang_message_cursor_row,
        state.hang_message_cursor_col,
        theme,
    );
}

fn render_multiline_editor_generic(
    buf: &mut Buffer,
    cx: u16,
    y: u16,
    cw: usize,
    height: usize,
    lines: &[String],
    cursor_row: usize,
    cursor_col: usize,
    theme: &ThemeColors,
) {
    // Build visual rows with wrapping (same approach as context_input.rs)
    let mut vrows: Vec<(usize, usize, usize)> = Vec::new();
    let mut cursor_vrow: usize = 0;

    for (li, line_text) in lines.iter().enumerate() {
        if cw == 0 {
            vrows.push((li, 0, line_text.len()));
            continue;
        }
        if line_text.is_empty() {
            if li == cursor_row {
                cursor_vrow = vrows.len();
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
            if li == cursor_row
                && cursor_col >= chunk_start
                && (cursor_col < end || (cursor_col == end && end == line_text.len()))
            {
                cursor_vrow = vrows.len();
            }
            vrows.push((li, chunk_start, end));
            pos = end;
        }
    }

    // Scroll so cursor is visible
    let visible_start = if cursor_vrow >= height {
        cursor_vrow - height + 1
    } else {
        0
    };

    for vi in 0..height {
        let vrow_idx = visible_start + vi;
        let row_y = y + vi as u16;
        if vrow_idx >= vrows.len() {
            break;
        }

        let (li, byte_start, byte_end) = vrows[vrow_idx];
        let chunk = &lines[li][byte_start..byte_end];
        let is_cursor_vrow = vrow_idx == cursor_vrow;

        if is_cursor_vrow {
            let cursor_byte = cursor_col.min(byte_end).saturating_sub(byte_start);
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
            Paragraph::new(line).render(Rect::new(cx, row_y, cw as u16, 1), buf);
        } else {
            buf.set_string(cx, row_y, chunk, Style::default().fg(theme.text));
        }
    }
}
