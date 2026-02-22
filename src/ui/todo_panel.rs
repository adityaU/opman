use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::App;
use crate::theme::ThemeColors;

pub fn render_todo_panel(app: &App, area: Rect, buf: &mut Buffer) {
    let state = match &app.todo_panel {
        Some(s) => s,
        None => return,
    };
    let theme = &app.theme;

    let popup_width = 60u16.min(area.width.saturating_sub(2));
    let max_list = (area.height / 2).saturating_sub(6);
    let list_rows = (state.todos.len() as u16
        + if state.editing.as_ref().map_or(false, |e| e.index.is_none()) {
            1
        } else {
            0
        })
    .max(3)
    .min(max_list.max(3));
    let popup_height = (list_rows + 6).min(area.height.saturating_sub(2));
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };
    super::render_overlay_dim(area, buf);
    Clear.render(popup_area, buf);

    let block = Block::default().style(Style::default().bg(theme.background_panel));
    let panel_inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let inner = Rect {
        x: panel_inner.x + 2,
        y: panel_inner.y + 1,
        width: panel_inner.width.saturating_sub(4),
        height: panel_inner.height.saturating_sub(1),
    };

    if inner.height < 4 {
        return;
    }

    let short_id = if state.session_id.len() > 12 {
        &state.session_id[..12]
    } else {
        &state.session_id
    };
    let title_text = format!("Todos — {}", short_id);
    let hint_text = "q";
    let fill_len = inner.width as usize
        - title_text.len().min(inner.width as usize)
        - hint_text.len().min(inner.width as usize);
    let title_line = Line::from(vec![
        Span::styled(
            &title_text,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(fill_len.max(1))),
        Span::styled(hint_text, Style::default().fg(theme.text_muted)),
    ]);
    Paragraph::new(title_line).render(
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
        buf,
    );

    let status_y = inner.y + 1;
    let sep1_y = inner.y + 2;
    let list_y = inner.y + 3;
    let hint_y = inner.y + inner.height.saturating_sub(1);
    let sep2_y = hint_y.saturating_sub(1);
    let list_height = sep2_y.saturating_sub(list_y);

    render_status_line(buf, inner.x, status_y, inner.width, state, theme);

    let sep: String = "─".repeat(inner.width as usize);
    buf.set_string(
        inner.x,
        sep1_y,
        &sep,
        Style::default().fg(theme.border_subtle),
    );
    if sep2_y > list_y {
        buf.set_string(
            inner.x,
            sep2_y,
            &sep,
            Style::default().fg(theme.border_subtle),
        );
    }

    if list_height > 0 {
        render_todo_list(buf, inner.x, list_y, inner.width, list_height, state, theme);
    }

    render_hint_line(buf, inner.x, hint_y, inner.width, state, theme);
}

fn render_status_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &crate::app::TodoPanelState,
    theme: &ThemeColors,
) {
    let total = state.todos.len();
    let completed = state
        .todos
        .iter()
        .filter(|t| t.status == "completed")
        .count();
    let in_progress = state
        .todos
        .iter()
        .filter(|t| t.status == "in_progress")
        .count();
    let pending = state.todos.iter().filter(|t| t.status == "pending").count();

    let line = Line::from(vec![
        Span::styled(format!(" {} total", total), Style::default().fg(theme.text)),
        Span::styled("  \u{25cf} ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{}", completed),
            Style::default().fg(theme.text_muted),
        ),
        Span::styled("  \u{25d1} ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("{}", in_progress),
            Style::default().fg(theme.text_muted),
        ),
        Span::styled("  \u{25cb} ", Style::default().fg(theme.text_muted)),
        Span::styled(
            format!("{}", pending),
            Style::default().fg(theme.text_muted),
        ),
    ]);
    Paragraph::new(line).render(
        Rect {
            x,
            y,
            width,
            height: 1,
        },
        buf,
    );
}

fn render_todo_list(
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
        let priority_color = priority_color(&todo.priority, is_selected, theme);
        buf.set_string(
            x + 3,
            row,
            priority_arrow(&todo.priority),
            Style::default().fg(priority_color).bg(bg),
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

                let priority_color = priority_color(&editing.priority, true, theme);
                buf.set_string(
                    x + 3,
                    row,
                    priority_arrow(&editing.priority),
                    Style::default().fg(priority_color).bg(bg),
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

fn render_hint_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &crate::app::TodoPanelState,
    theme: &ThemeColors,
) {
    let hints = if state.editing.is_some() {
        vec![("Enter", "confirm"), ("Esc", "cancel")]
    } else {
        vec![
            ("Space", "toggle"),
            ("n", "new"),
            ("e", "edit"),
            ("d", "delete"),
            ("p", "priority"),
            ("S+K/J", "reorder"),
            ("q", "close"),
        ]
    };

    let mut spans = Vec::new();
    spans.push(Span::raw(" "));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(theme.text_muted)));
        }
        spans.push(Span::styled(
            *key,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(":{}", desc),
            Style::default().fg(theme.text_muted),
        ));
    }
    let line = Line::from(spans);
    let area = Rect {
        x,
        y,
        width,
        height: 1,
    };
    Paragraph::new(line).render(area, buf);
}

fn status_icon_color<'a>(
    todo: &crate::app::TodoItem,
    theme: &'a ThemeColors,
    is_selected: bool,
) -> (&'static str, Color) {
    match todo.status.as_str() {
        "completed" => ("\u{25cf}", Color::Green),
        "in_progress" => ("\u{25d1}", Color::Yellow),
        "cancelled" => ("\u{2715}", Color::Red),
        _ => (
            "\u{25cb}",
            if is_selected {
                theme.background
            } else {
                theme.text_muted
            },
        ),
    }
}

fn priority_arrow(priority: &str) -> &'static str {
    match priority {
        "high" => "↑",
        "medium" => "→",
        "low" => "↓",
        _ => "·",
    }
}

fn priority_color(priority: &str, is_selected: bool, _theme: &ThemeColors) -> Color {
    match priority {
        "high" => Color::Red,
        "medium" => Color::Yellow,
        "low" => Color::Green,
        _ => {
            if is_selected {
                Color::White
            } else {
                Color::DarkGray
            }
        }
    }
}
