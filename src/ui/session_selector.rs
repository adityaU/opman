use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::{App, SessionSelectorState};
use crate::theme::ThemeColors;

fn format_relative_time(updated: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let updated_secs = if updated > 10000000000 {
        updated / 1000
    } else {
        updated
    };

    let diff = now.saturating_sub(updated_secs);
    match diff {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{}m ago", diff / 60),
        3600..=86399 => format!("{}h ago", diff / 3600),
        _ => format!("{}d ago", diff / 86400),
    }
}

pub fn render_session_selector(app: &App, area: Rect, buf: &mut Buffer) {
    let state = match &app.session_selector {
        Some(s) => s,
        None => return,
    };

    let theme = &app.theme;

    let popup_width = 80u16.min(area.width.saturating_sub(2));
    let list_max = (area.height / 2).saturating_sub(6);
    let popup_height = (list_max + 6).max(8).min(area.height);
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

    let title_span = Span::styled(
        "Select Session",
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let esc_span = Span::styled("esc", Style::default().fg(theme.text_muted));
    let fill_len = (inner.width as usize).saturating_sub(14 + 3);
    let fill = " ".repeat(fill_len);
    let title_line = Line::from(vec![title_span, Span::raw(fill), esc_span]);
    buf.set_line(inner.x, inner.y, &title_line, inner.width);

    let input_y = inner.y + 1;
    let separator_y = input_y + 1;
    let hint_y = inner.y + inner.height.saturating_sub(1);
    let results_y = separator_y + 1;
    let results_height = hint_y.saturating_sub(results_y);

    // Input line
    render_input_line(buf, inner.x, input_y, inner.width, state, theme);

    // Separator
    let sep: String = "─".repeat(inner.width as usize);
    buf.set_string(
        inner.x,
        separator_y,
        &sep,
        Style::default().fg(theme.border_subtle),
    );

    // Results area
    if results_height > 0 {
        let results_area = Rect {
            x: inner.x,
            y: results_y,
            width: inner.width,
            height: results_height,
        };
        render_results(buf, results_area, state, theme);
    }

    // Hint line
    render_hint_line(buf, inner.x, hint_y, inner.width, theme);
}

fn render_input_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &SessionSelectorState,
    theme: &ThemeColors,
) {
    let prompt = "> ";
    buf.set_string(
        x,
        y,
        prompt,
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD),
    );

    let query_x = x + prompt.len() as u16;
    buf.set_string(query_x, y, &state.query, Style::default().fg(theme.text));

    // Block cursor
    let cursor_x = query_x + state.cursor_pos as u16;
    if cursor_x < x + width {
        let cursor_char = state.query[state.cursor_pos..]
            .chars()
            .next()
            .unwrap_or(' ');
        buf.set_string(
            cursor_x,
            y,
            cursor_char.to_string(),
            Style::default()
                .fg(theme.background)
                .bg(theme.text)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn render_results(buf: &mut Buffer, area: Rect, state: &SessionSelectorState, theme: &ThemeColors) {
    if state.filtered.is_empty() {
        if !state.query.is_empty() {
            buf.set_string(
                area.x + 2,
                area.y + area.height / 2,
                "No matching sessions",
                Style::default().fg(theme.text_muted),
            );
        } else {
            buf.set_string(
                area.x + 2,
                area.y + area.height / 2,
                "No sessions found",
                Style::default().fg(theme.text_muted),
            );
        }
        return;
    }

    let visible_count = area.height as usize;
    let selected = state.selected.min(state.filtered.len().saturating_sub(1));

    // Scroll adjustment
    let scroll_offset = if selected >= state.scroll_offset + visible_count {
        selected - visible_count + 1
    } else if selected < state.scroll_offset {
        selected
    } else {
        state.scroll_offset
    };

    let end = state.filtered.len().min(scroll_offset + visible_count);

    for (row_idx, i) in (scroll_offset..end).enumerate() {
        let row = area.y + row_idx as u16;
        if row >= area.y + area.height {
            break;
        }

        let entry_idx = state.filtered[i];
        let entry = &state.entries[entry_idx];
        let is_selected = i == selected;

        // Clear the row with appropriate background
        let row_bg = if is_selected {
            Style::default()
                .bg(theme.primary)
                .fg(theme.background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, row, &blank, row_bg);

        // Build the line: {project_name} / {session_title}    {relative_time}
        let session_title = if entry.session.title.is_empty() {
            // Use first 8 chars of ID as fallback
            if entry.session.id.len() > 8 {
                format!("{}…", &entry.session.id[..8])
            } else {
                entry.session.id.clone()
            }
        } else {
            entry.session.title.clone()
        };

        let relative_time = format_relative_time(entry.session.time.updated);

        // Calculate available width for the main content
        let time_width = relative_time.len() + 2; // padding
        let content_max = (area.width as usize).saturating_sub(time_width + 2); // 2 for left margin

        let project_part = &entry.project_name;
        let separator = " / ";
        let _full_content = format!("{}{}{}", project_part, separator, session_title);

        // Render project name part
        let margin = 2u16;
        let proj_style = if is_selected {
            row_bg.add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD)
        };
        let project_display_len = project_part.len().min(content_max);
        buf.set_string(
            area.x + margin,
            row,
            &project_part[..project_display_len],
            proj_style,
        );

        // Render separator " / "
        let sep_x = area.x + margin + project_display_len as u16;
        if (sep_x as usize) < (area.x + area.width) as usize {
            let sep_style = if is_selected {
                row_bg
            } else {
                Style::default().fg(theme.text_muted)
            };
            let sep_len = separator.len().min((area.x + area.width - sep_x) as usize);
            buf.set_string(sep_x, row, &separator[..sep_len], sep_style);
        }

        // Render session title
        let title_x = sep_x + separator.len() as u16;
        if title_x < area.x + area.width {
            let title_style = if is_selected {
                row_bg
            } else {
                Style::default().fg(theme.text)
            };
            let remaining = (area.x + area.width).saturating_sub(title_x) as usize;
            let title_max = remaining.saturating_sub(time_width);
            let title_display = if session_title.len() > title_max {
                let avail = title_max.saturating_sub(1);
                format!("{}…", &session_title[..avail.min(session_title.len())])
            } else {
                session_title.clone()
            };
            buf.set_string(title_x, row, &title_display, title_style);
        }

        // Render relative time (right-aligned)
        let time_style = if is_selected {
            row_bg
        } else {
            Style::default().fg(theme.text_muted)
        };
        let time_x = area.x + area.width - relative_time.len() as u16 - 1;
        if time_x > title_x {
            buf.set_string(time_x, row, &relative_time, time_style);
        }
    }
}

fn render_hint_line(buf: &mut Buffer, x: u16, y: u16, width: u16, theme: &ThemeColors) {
    let spans = vec![
        Span::styled("  ↑↓", Style::default().fg(theme.accent)),
        Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
        Span::styled("⏎", Style::default().fg(theme.accent)),
        Span::styled(" select  ", Style::default().fg(theme.text_muted)),
        Span::styled("esc", Style::default().fg(theme.accent)),
        Span::styled(" close", Style::default().fg(theme.text_muted)),
    ];

    let line = Line::from(spans);
    let hint_area = Rect {
        x,
        y,
        width,
        height: 1,
    };
    Paragraph::new(line).render(hint_area, buf);
}
