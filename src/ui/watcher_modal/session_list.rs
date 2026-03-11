use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::WatcherField;
use crate::theme::ThemeColors;

pub(super) fn render_session_list(
    buf: &mut Buffer,
    area: Rect,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    if area.height == 0 || area.width < 4 {
        return;
    }

    // Label
    let label = "Sessions";
    let label_style = if state.active_field == WatcherField::SessionList {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.text_muted)
            .add_modifier(Modifier::BOLD)
    };
    buf.set_string(area.x + 1, area.y, label, label_style);

    let list_y = area.y + 1;
    let list_height = area.height.saturating_sub(1) as usize;

    if state.sessions.is_empty() {
        buf.set_string(
            area.x + 1,
            list_y,
            "No sessions",
            Style::default().fg(theme.text_muted),
        );
        return;
    }

    let selected = state
        .selected_session_idx
        .min(state.sessions.len().saturating_sub(1));

    // Scroll
    let scroll = if selected >= state.session_scroll + list_height {
        selected - list_height + 1
    } else if selected < state.session_scroll {
        selected
    } else {
        state.session_scroll
    };

    let end = state.sessions.len().min(scroll + list_height);
    let max_w = area.width.saturating_sub(2) as usize;

    // Track grouping: current, active, watched
    let mut last_group: Option<&str> = None;
    let mut row_offset: u16 = 0;

    for i in scroll..end {
        let row_y = list_y + row_offset;
        if row_y >= area.y + area.height {
            break;
        }

        let entry = &state.sessions[i];

        // Determine group
        let group = if entry.is_current {
            "Current"
        } else if entry.is_active {
            "Active"
        } else if entry.has_watcher {
            "Watched"
        } else {
            "Other"
        };

        // Render group header if changed
        if last_group != Some(group) {
            if last_group.is_some() && row_y < area.y + area.height {
                // blank separator line between groups
                row_offset += 1;
                let sep_row = list_y + row_offset;
                if sep_row >= area.y + area.height {
                    break;
                }
            }
            let hdr_y = list_y + row_offset;
            if hdr_y >= area.y + area.height {
                break;
            }
            buf.set_string(
                area.x + 1,
                hdr_y,
                group,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            );
            row_offset += 1;
            last_group = Some(group);
        }

        let entry_y = list_y + row_offset;
        if entry_y >= area.y + area.height {
            break;
        }

        let is_selected = i == selected && state.active_field == WatcherField::SessionList;

        // Row background
        let row_style = if is_selected {
            Style::default()
                .bg(theme.primary)
                .fg(theme.background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, entry_y, &blank, row_style);

        // Indicator
        let indicator = if entry.has_watcher {
            "● "
        } else if is_selected {
            "> "
        } else {
            "  "
        };
        let ind_style = if is_selected {
            row_style
        } else if entry.has_watcher {
            Style::default().fg(theme.success)
        } else {
            row_style
        };
        buf.set_string(area.x + 1, entry_y, indicator, ind_style);

        // Project name + title
        let display = if entry.title.is_empty() {
            entry.project_name.clone()
        } else {
            let proj = &entry.project_name;
            let avail = max_w.saturating_sub(2); // indicator width
            let full = format!("{}/{}", proj, entry.title);
            if full.len() > avail {
                format!("{}...", &full[..avail.saturating_sub(3).min(full.len())])
            } else {
                full
            }
        };
        buf.set_string(area.x + 3, entry_y, &display, row_style);

        row_offset += 1;
    }
}
