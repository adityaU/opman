use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::ThemeColors;

pub(super) fn render_routine_list(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    state: &crate::app::RoutinePanelState,
    theme: &ThemeColors,
) {
    if state.loading {
        let msg = "Loading routines...";
        let msg_x = x + (width.saturating_sub(msg.len() as u16)) / 2;
        let msg_y = y + height / 2;
        buf.set_string(msg_x, msg_y, msg, Style::default().fg(theme.text_muted));
        return;
    }

    if state.routines.is_empty() {
        let empty_msg = "No routines configured";
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

    let scroll_offset = if state.selected >= state.scroll_offset + visible_count {
        state.selected + 1 - visible_count
    } else if state.selected < state.scroll_offset {
        state.selected
    } else {
        state.scroll_offset
    };

    let end = (scroll_offset + visible_count).min(state.routines.len());

    for (i, routine_idx) in (scroll_offset..end).enumerate() {
        let row = y + i as u16;
        let routine = &state.routines[routine_idx];
        let is_selected = routine_idx == state.selected;
        let is_running = state.running.as_deref() == Some(&routine.id);

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

        // Enabled/disabled indicator
        let (status_icon, status_color) = if is_running {
            ("\u{25b6}", Color::Yellow) // ▶ running
        } else if routine.enabled {
            ("\u{25cf}", Color::Green) // ● enabled
        } else {
            ("\u{25cb}", Color::DarkGray) // ○ disabled
        };
        buf.set_string(
            x + 1,
            row,
            status_icon,
            Style::default().fg(status_color).bg(bg),
        );

        // Trigger type indicator
        let trigger_icon = match routine.trigger.as_str() {
            "scheduled" => "\u{23f0}",       // ⏰
            "manual" => "\u{270b}",          // ✋
            "on_session_idle" => "\u{23f8}", // ⏸
            "daily_summary" => "\u{2600}",   // ☀
            _ => "\u{2022}",                 // •
        };
        let trigger_color = if is_selected { fg } else { Color::Cyan };
        buf.set_string(
            x + 3,
            row,
            trigger_icon,
            Style::default().fg(trigger_color).bg(bg),
        );

        // Name
        let name_start = 5u16;
        // Reserve space for right-side info
        let right_info = format_right_info(routine);
        let right_width = right_info.len();
        let max_name_width = (width as usize)
            .saturating_sub(name_start as usize)
            .saturating_sub(right_width + 2);

        let name = if routine.name.chars().count() > max_name_width {
            let truncated: String = routine
                .name
                .chars()
                .take(max_name_width.saturating_sub(1))
                .collect();
            format!("{}\u{2026}", truncated)
        } else {
            routine.name.clone()
        };

        let name_style = if is_selected {
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD)
        } else if !routine.enabled {
            Style::default().fg(theme.text_muted).bg(bg)
        } else {
            Style::default().fg(fg).bg(bg)
        };
        buf.set_string(x + name_start, row, &name, name_style);

        // Right-aligned info (cron expr or action type)
        if !right_info.is_empty() && width > name_start + right_info.len() as u16 + 2 {
            let info_x = x + width - right_info.len() as u16 - 1;
            let info_style = if is_selected {
                Style::default().fg(fg).bg(bg)
            } else {
                Style::default().fg(theme.text_muted).bg(bg)
            };
            buf.set_string(info_x, row, &right_info, info_style);
        }
    }
}

fn format_right_info(routine: &crate::app::RoutineItem) -> String {
    // For scheduled routines, show next run time if available
    if routine.trigger == "scheduled" {
        if let Some(ref next) = routine.next_run_at {
            if !next.is_empty() {
                // Try to show a compact time: just the time portion or short date+time
                if let Some(t_idx) = next.find('T') {
                    let time_part = &next[t_idx + 1..];
                    // Take HH:MM from the time portion
                    let hm: String = time_part.chars().take(5).collect();
                    if let Some(ref cron) = routine.cron_expr {
                        return format!("{} \u{2192} {}", cron, hm);
                    }
                    return format!("next {}", hm);
                }
            }
        }
        if let Some(ref cron) = routine.cron_expr {
            if !cron.is_empty() {
                return cron.clone();
            }
        }
    }
    match routine.action.as_str() {
        "send_message" => "send msg".to_string(),
        "review_mission" => "review".to_string(),
        "open_inbox" => "inbox".to_string(),
        "open_activity_feed" => "feed".to_string(),
        _ => routine.action.clone(),
    }
}
