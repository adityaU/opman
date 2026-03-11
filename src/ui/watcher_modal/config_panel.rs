use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::WatcherField;
use crate::theme::ThemeColors;

use super::editor::render_multiline_editor;
use super::hang_section::render_hang_section;

pub(super) fn render_config_panel(
    buf: &mut Buffer,
    area: Rect,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    if area.height < 4 || area.width < 10 {
        return;
    }

    let cx = area.x + 1;
    let cw = area.width.saturating_sub(2) as usize;
    let mut y = area.y;

    // Check if a session is selected
    if state.sessions.is_empty() {
        buf.set_string(
            cx,
            y,
            "Select a session",
            Style::default().fg(theme.text_muted),
        );
        return;
    }

    let selected_entry = &state.sessions[state
        .selected_session_idx
        .min(state.sessions.len().saturating_sub(1))];

    // Session info header
    let info = format!("{} / {}", selected_entry.project_name, selected_entry.title);
    let info_display = if info.len() > cw {
        format!(
            "{}...",
            crate::util::truncate_str(&info, cw.saturating_sub(3))
        )
    } else {
        info
    };
    buf.set_string(
        cx,
        y,
        &info_display,
        Style::default()
            .fg(theme.secondary)
            .add_modifier(Modifier::BOLD),
    );
    y += 1;

    // ── Continuation Message ──
    let msg_label = "Continuation Message:";
    let msg_label_style = if state.active_field == WatcherField::Message {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    buf.set_string(cx, y, msg_label, msg_label_style);
    y += 1;

    // Message editor area (up to 4 rows)
    let msg_rows = 4u16.min((area.y + area.height).saturating_sub(y + 14)); // leave room for other fields + hang detection
    let msg_area_height = msg_rows as usize;

    if state.active_field == WatcherField::Message {
        render_multiline_editor(buf, cx, y, cw, msg_area_height, state, theme);
    } else {
        // Render message text (read-only view)
        for (i, line) in state.message_lines.iter().enumerate().take(msg_area_height) {
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
        if state.message_lines.is_empty()
            || (state.message_lines.len() == 1 && state.message_lines[0].is_empty())
        {
            buf.set_string(
                cx,
                y,
                "(empty - type a message)",
                Style::default().fg(theme.text_muted),
            );
        }
    }
    y += msg_rows;

    // ── Include Original Toggle ──
    y += 1; // spacing
    if y >= area.y + area.height - 2 {
        return;
    }
    let check_char = if state.include_original { "[x]" } else { "[ ]" };
    let toggle_focused = state.active_field == WatcherField::IncludeOriginal;
    let check_style = if toggle_focused {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.accent)
    };
    let label_style = if toggle_focused {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    buf.set_string(cx, y, check_char, check_style);
    buf.set_string(cx + 4, y, "Include original message", label_style);
    y += 1;

    // ── Original Message List (only if toggled on) ──
    if state.include_original {
        y = render_original_message_list(buf, area, cx, y, cw, state, theme);
    }

    // ── Timeout Input ──
    if y >= area.y + area.height - 1 {
        return;
    }
    y += if state.include_original { 0 } else { 1 }; // spacing
    if y >= area.y + area.height - 1 {
        return;
    }
    y = render_timeout_input(buf, area, cx, y, cw, state, theme);

    // ── Hang Detection Section ──
    render_hang_section(buf, area, cx, y, cw, state, theme);
}

fn render_original_message_list(
    buf: &mut Buffer,
    area: Rect,
    cx: u16,
    mut y: u16,
    cw: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) -> u16 {
    if y >= area.y + area.height - 2 {
        return y;
    }
    let orig_label = "Select message:";
    let orig_label_style = if state.active_field == WatcherField::OriginalMessageList {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    buf.set_string(cx, y, orig_label, orig_label_style);
    y += 1;

    let remaining_for_msgs = (area.y + area.height).saturating_sub(y + 2) as usize;
    let msg_list_height = remaining_for_msgs.min(4);

    if state.session_messages.is_empty() {
        buf.set_string(
            cx,
            y,
            "(loading messages...)",
            Style::default().fg(theme.text_muted),
        );
        y += 1;
    } else {
        let sel = state
            .selected_message_idx
            .min(state.session_messages.len().saturating_sub(1));
        let scroll = if sel >= state.message_scroll + msg_list_height {
            sel - msg_list_height + 1
        } else if sel < state.message_scroll {
            sel
        } else {
            state.message_scroll
        };

        let end = state.session_messages.len().min(scroll + msg_list_height);
        for i in scroll..end {
            if y >= area.y + area.height - 1 {
                break;
            }
            let msg = &state.session_messages[i];
            let is_sel = i == sel && state.active_field == WatcherField::OriginalMessageList;

            let row_style = if is_sel {
                Style::default()
                    .bg(theme.primary)
                    .fg(theme.background)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            let blank: String = " ".repeat(cw);
            buf.set_string(cx, y, &blank, row_style);

            // Truncate message preview
            let preview = msg.text.replace('\n', " ");
            let preview = if preview.len() > cw.saturating_sub(2) {
                format!(
                    "{}...",
                    crate::util::truncate_str(&preview, cw.saturating_sub(5))
                )
            } else {
                preview
            };
            buf.set_string(cx + 1, y, &preview, row_style);
            y += 1;
        }
    }
    y
}

fn render_timeout_input(
    buf: &mut Buffer,
    area: Rect,
    cx: u16,
    y: u16,
    _cw: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) -> u16 {
    let timeout_focused = state.active_field == WatcherField::TimeoutInput;
    let timeout_label = "Idle timeout (seconds): ";
    let timeout_label_style = if timeout_focused {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    buf.set_string(cx, y, timeout_label, timeout_label_style);

    let val_x = cx + timeout_label.len() as u16;
    let val_text = &state.timeout_input;
    buf.set_string(val_x, y, val_text, Style::default().fg(theme.text));

    // Block cursor for timeout input
    if timeout_focused {
        let cursor_x = val_x + val_text.len() as u16;
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

    y + 2
}
