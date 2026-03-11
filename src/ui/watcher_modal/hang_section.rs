use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::WatcherField;
use crate::theme::ThemeColors;

use super::editor::render_hang_message_editor;

pub(super) fn render_hang_section(
    buf: &mut Buffer,
    area: Rect,
    cx: u16,
    mut y: u16,
    cw: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    if y >= area.y + area.height - 1 {
        return;
    }
    let sep_label = "─── Hang Detection ───";
    buf.set_string(cx, y, sep_label, Style::default().fg(theme.text_muted));
    y += 1;
    if y >= area.y + area.height - 1 {
        return;
    }

    // ── Hang Message ──
    let hang_msg_label = "Hang Retry Message:";
    let hang_msg_label_style = if state.active_field == WatcherField::HangMessage {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    buf.set_string(cx, y, hang_msg_label, hang_msg_label_style);
    y += 1;

    // Hang message editor area (up to 3 rows)
    let hang_msg_rows = 3u16.min(area.y + area.height - y - 3);
    let hang_msg_area_height = hang_msg_rows as usize;

    if state.active_field == WatcherField::HangMessage {
        render_hang_message_editor(buf, cx, y, cw, hang_msg_area_height, state, theme);
    } else {
        // Render hang message text (read-only view)
        for (i, line) in state
            .hang_message_lines
            .iter()
            .enumerate()
            .take(hang_msg_area_height)
        {
            let row_y = y + i as u16;
            if row_y >= area.y + area.height {
                break;
            }
            let display = if line.len() > cw {
                &line[..cw]
            } else {
                line.as_str()
            };
            buf.set_string(cx, row_y, display, Style::default().fg(theme.text));
        }
        if state.hang_message_lines.is_empty()
            || (state.hang_message_lines.len() == 1 && state.hang_message_lines[0].is_empty())
        {
            buf.set_string(
                cx,
                y,
                "(empty - type a message)",
                Style::default().fg(theme.text_muted),
            );
        }
    }
    y += hang_msg_rows;

    // ── Hang Timeout Input ──
    if y >= area.y + area.height - 1 {
        return;
    }
    y += 1; // spacing
    if y >= area.y + area.height - 1 {
        return;
    }
    let hang_timeout_focused = state.active_field == WatcherField::HangTimeoutInput;
    let hang_timeout_label = "Hang timeout (seconds): ";
    let hang_timeout_label_style = if hang_timeout_focused {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    buf.set_string(cx, y, hang_timeout_label, hang_timeout_label_style);

    let hang_val_x = cx + hang_timeout_label.len() as u16;
    let hang_val_text = &state.hang_timeout_input;
    buf.set_string(
        hang_val_x,
        y,
        hang_val_text,
        Style::default().fg(theme.text),
    );

    // Block cursor for hang timeout input
    if hang_timeout_focused {
        let cursor_x = hang_val_x + hang_val_text.len() as u16;
        if cursor_x < area.x + area.width {
            buf.set_string(
                cursor_x,
                y,
                " ",
                Style::default()
                    .bg(theme.text)
                    .fg(theme.background)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}
