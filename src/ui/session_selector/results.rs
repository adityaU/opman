use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::SessionSelectorState;
use crate::theme::ThemeColors;

use super::format_relative_time;

pub(super) fn render_results(
    buf: &mut Buffer,
    area: Rect,
    state: &SessionSelectorState,
    theme: &ThemeColors,
) {
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
                format!("{}…", crate::util::truncate_str(&session_title, avail))
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
