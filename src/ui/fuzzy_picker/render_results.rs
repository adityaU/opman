use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::theme::ThemeColors;

use super::FuzzyPickerState;

pub(super) fn render_results(
    buf: &mut Buffer,
    area: Rect,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    if state.query.is_empty() && !state.existing_projects.is_empty() {
        render_existing_projects(buf, area, state, theme);
        return;
    }

    let snapshot = state.matcher.snapshot();
    let matched = snapshot.matched_item_count();

    if matched == 0 {
        // Show "no results" message
        if !state.query.is_empty() {
            buf.set_string(
                area.x + 2,
                area.y + area.height / 2,
                "No matching directories",
                Style::default().fg(theme.text_muted),
            );
        }
        return;
    }

    let visible_count = area.height as u32;
    let selected = state.selected.min(matched.saturating_sub(1));

    // Ensure selected item is visible (scroll adjustment)
    let scroll_offset = if selected >= state.scroll_offset + visible_count {
        selected - visible_count + 1
    } else if selected < state.scroll_offset {
        selected
    } else {
        state.scroll_offset
    };

    // Render items bottom-up (fzf style: most relevant at bottom, near input)
    let items: Vec<_> = snapshot
        .matched_items(scroll_offset..matched.min(scroll_offset + visible_count))
        .collect();

    for (i, item) in items.iter().enumerate() {
        let row = area.y + area.height.saturating_sub(1) - i as u16;
        if row < area.y {
            break;
        }

        let display = item.matcher_columns[0].to_string();
        let is_selected = (scroll_offset + i as u32) == selected;

        let style = if is_selected {
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        // Clear the row
        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, row, &blank, style);

        // Render path with indicator
        let indicator = if is_selected { "> " } else { "  " };
        buf.set_string(area.x, row, indicator, style);

        let max_path_width = (area.width as usize).saturating_sub(2);
        let truncated = if display.len() > max_path_width {
            format!("...{}", &display[display.len() - max_path_width + 3..])
        } else {
            display
        };
        buf.set_string(area.x + 2, row, &truncated, style);
    }
}

fn render_existing_projects(
    buf: &mut Buffer,
    area: Rect,
    state: &FuzzyPickerState,
    theme: &ThemeColors,
) {
    let count = state.existing_projects.len() as u32;
    if count == 0 {
        return;
    }

    let visible_count = area.height as u32;
    let selected = state.selected.min(count.saturating_sub(1));

    let scroll_offset = if selected >= state.scroll_offset + visible_count {
        selected - visible_count + 1
    } else if selected < state.scroll_offset {
        selected
    } else {
        state.scroll_offset
    };

    let end = count.min(scroll_offset + visible_count);

    for i in scroll_offset..end {
        let row_idx = i - scroll_offset;
        let row = area.y + area.height.saturating_sub(1) - row_idx as u16;
        if row < area.y {
            break;
        }

        let (ref display, _) = state.existing_projects[i as usize];
        let is_selected = i == selected;

        let style = if is_selected {
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        // Clear the row
        let blank: String = " ".repeat(area.width as usize);
        buf.set_string(area.x, row, &blank, style);

        // Render path with indicator
        let indicator = if is_selected { "> " } else { "  " };
        buf.set_string(area.x, row, indicator, style);

        let max_path_width = (area.width as usize).saturating_sub(2);
        let truncated = if display.len() > max_path_width {
            format!("...{}", &display[display.len() - max_path_width + 3..])
        } else {
            display.clone()
        };
        buf.set_string(area.x + 2, row, &truncated, style);
    }
}
