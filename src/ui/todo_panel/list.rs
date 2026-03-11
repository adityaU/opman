use ratatui::buffer::Buffer;
use ratatui::style::Style;

use crate::theme::ThemeColors;

use super::{priority_arrow, priority_color, status_icon_color};

pub(super) fn render_todo_list(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    state: &crate::app::TodoPanelState,
    theme: &ThemeColors,
) {
    if state.todos.is_empty() {
        let empty_msg = "No todos for this session";
        let msg_x = x + (width.saturating_sub(empty_msg.len() as u16)) / 2;
        let msg_y = y + height / 2;
        buf.set_string(
            msg_x,
            msg_y,
            empty_msg,
            Style::default().fg(theme.text_muted),
        );
        return;
    }

    let visible_count = height as usize;

    // Adjust scroll_offset (we use the mutable state's scroll_offset via the const ref)
    let scroll_offset = if state.selected >= state.scroll_offset + visible_count {
        state.selected + 1 - visible_count
    } else if state.selected < state.scroll_offset {
        state.selected
    } else {
        state.scroll_offset
    };

    let end = (scroll_offset + visible_count).min(state.todos.len());

    for (i, todo_idx) in (scroll_offset..end).enumerate() {
        let row = y + i as u16;
        let todo = &state.todos[todo_idx];
        let is_selected = todo_idx == state.selected;

        // Check if we're editing this todo
        let is_editing = state
            .editing
            .as_ref()
            .map_or(false, |e| e.index == Some(todo_idx));

        let bg = if is_selected {
            theme.primary
        } else {
            theme.background
        };
        let fg = if is_selected {
            theme.background
        } else {
            theme.text
        };

        // Clear row
        let blank: String = " ".repeat(width as usize);
        buf.set_string(x, row, &blank, Style::default().bg(bg));

        // Status icon
        let (status_icon, status_color) = status_icon_color(todo, theme, is_selected);
        buf.set_string(
            x + 1,
            row,
            status_icon,
            Style::default().fg(status_color).bg(bg),
        );

        // Priority indicator
        let pcolor = priority_color(&todo.priority, is_selected, theme);
        buf.set_string(
            x + 3,
            row,
            priority_arrow(&todo.priority),
            Style::default().fg(pcolor).bg(bg),
        );

        // Content
        if is_editing {
            if let Some(ref editing) = state.editing {
                let max_content_width = (width as usize).saturating_sub(6);
                let scroll_start = if editing.buffer.len() > max_content_width {
                    editing.buffer.len() - max_content_width
                } else {
                    0
                };
                let display = &editing.buffer[scroll_start..];
                buf.set_string(x + 5, row, display, Style::default().fg(fg).bg(bg));

                let display_cursor_pos = editing.cursor_pos.saturating_sub(scroll_start);
                let cursor_x = x + 5 + display_cursor_pos as u16;
                if cursor_x < x + width {
                    let cursor_char = editing.buffer[editing.cursor_pos..]
                        .chars()
                        .next()
                        .unwrap_or(' ');
                    buf.set_string(
                        cursor_x,
                        row,
                        cursor_char.to_string(),
                        Style::default().fg(bg).bg(fg),
                    );
                }
            }
        } else {
            let max_content_width = (width as usize).saturating_sub(6);
            let content = if todo.content.len() > max_content_width {
                format!(
                    "{}\u{2026}",
                    &todo.content[..max_content_width.saturating_sub(1)]
                )
            } else {
                todo.content.clone()
            };
            buf.set_string(x + 5, row, &content, Style::default().fg(fg).bg(bg));
        }
    }

    // If adding a new todo (editing.index == None), render it at the bottom as an extra row
    if let Some(ref editing) = state.editing {
        if editing.index.is_none() {
            // Find the row to render the new entry
            let new_row_idx = state.todos.len();
            if new_row_idx >= scroll_offset && new_row_idx < scroll_offset + visible_count {
                let row = y + (new_row_idx - scroll_offset) as u16;
                let bg = theme.primary;
                let fg = theme.background;
                let blank: String = " ".repeat(width as usize);
                buf.set_string(x, row, &blank, Style::default().bg(bg));
                buf.set_string(x + 1, row, "\u{25cb}", Style::default().fg(fg).bg(bg));

                let pcolor = priority_color(&editing.priority, true, theme);
                buf.set_string(
                    x + 3,
                    row,
                    priority_arrow(&editing.priority),
                    Style::default().fg(pcolor).bg(bg),
                );

                let max_content_width = (width as usize).saturating_sub(6);
                let scroll_start = if editing.buffer.len() > max_content_width {
                    editing.buffer.len() - max_content_width
                } else {
                    0
                };
                let display = &editing.buffer[scroll_start..];
                buf.set_string(x + 5, row, display, Style::default().fg(fg).bg(bg));

                let display_cursor_pos = editing.cursor_pos.saturating_sub(scroll_start);
                let cursor_x = x + 5 + display_cursor_pos as u16;
                if cursor_x < x + width {
                    let cursor_char = editing.buffer[editing.cursor_pos..]
                        .chars()
                        .next()
                        .unwrap_or(' ');
                    buf.set_string(
                        cursor_x,
                        row,
                        cursor_char.to_string(),
                        Style::default().fg(bg).bg(fg),
                    );
                }
            }
        }
    }
}
