mod list;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::App;
use crate::theme::ThemeColors;

use list::render_routine_list;

pub fn render_routine_panel(app: &App, area: Rect, buf: &mut Buffer) {
    let state = match &app.routine_panel {
        Some(s) => s,
        None => return,
    };
    let theme = &app.theme;

    // If editing, render the edit form instead of the normal panel
    if state.editing.is_some() {
        render_edit_form(state, area, buf, theme);
        return;
    }

    let show_detail = state.show_detail && !state.routines.is_empty();

    let popup_width = if show_detail {
        90u16.min(area.width.saturating_sub(2))
    } else {
        70u16.min(area.width.saturating_sub(2))
    };
    let max_list = (area.height / 2).saturating_sub(6);
    let list_rows = (state.routines.len() as u16).max(3).min(max_list.max(3));
    let detail_extra = if show_detail { 10u16 } else { 0 };
    let popup_height = (list_rows + 6 + detail_extra).min(area.height.saturating_sub(2));
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

    // Title bar
    let title_text = "Routines";
    let hint_text = "q";
    let fill_len = inner.width as usize
        - title_text.len().min(inner.width as usize)
        - hint_text.len().min(inner.width as usize);
    let title_line = Line::from(vec![
        Span::styled(
            title_text,
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

    // Status line
    let status_y = inner.y + 1;
    let sep1_y = inner.y + 2;
    let list_y = inner.y + 3;
    let hint_y = inner.y + inner.height.saturating_sub(1);

    // If detail view is active, split the area: list above, detail below
    let (list_bottom, detail_area) = if show_detail {
        // Reserve space for detail pane (hint + sep + detail rows)
        let detail_height = 9u16.min(inner.height.saturating_sub(8));
        let detail_sep_y = hint_y.saturating_sub(detail_height + 1);
        let detail_start_y = detail_sep_y + 1;
        let sep2_y = detail_sep_y.saturating_sub(1);
        (
            sep2_y,
            Some(Rect {
                x: inner.x,
                y: detail_start_y,
                width: inner.width,
                height: hint_y.saturating_sub(detail_start_y),
            }),
        )
    } else {
        let sep2_y = hint_y.saturating_sub(1);
        (sep2_y, None)
    };
    let list_height = list_bottom.saturating_sub(list_y);

    render_status_line(buf, inner.x, status_y, inner.width, state, theme);

    let sep: String = "\u{2500}".repeat(inner.width as usize);
    buf.set_string(
        inner.x,
        sep1_y,
        &sep,
        Style::default().fg(theme.border_subtle),
    );
    if list_bottom > list_y {
        buf.set_string(
            inner.x,
            list_bottom,
            &sep,
            Style::default().fg(theme.border_subtle),
        );
    }

    // Separator above hint
    if show_detail {
        let _hint_sep = hint_y.saturating_sub(1);
        // Separator between detail and hint is already the detail bottom edge
    }

    if list_height > 0 {
        render_routine_list(buf, inner.x, list_y, inner.width, list_height, state, theme);
    }

    // Detail pane
    if let Some(detail_rect) = detail_area {
        render_detail_pane(buf, detail_rect, state, theme);
    }

    render_hint_line(buf, inner.x, hint_y, inner.width, state, theme);

    // Delete confirmation overlay (rendered on top of everything else)
    if state.confirm_delete.is_some() {
        render_delete_confirmation(state, popup_area, buf, theme);
    }
}

/// Render the inline create/edit form.
fn render_edit_form(
    state: &crate::app::RoutinePanelState,
    area: Rect,
    buf: &mut Buffer,
    theme: &ThemeColors,
) {
    let edit = match &state.editing {
        Some(e) => e,
        None => return,
    };

    let is_create = edit.routine_id.is_none();
    let title = if is_create {
        "New Routine"
    } else {
        "Edit Routine"
    };

    // Form needs: title(1) + sep(1) + 6 fields * 2 lines each + sep(1) + hint(1) = ~16 rows
    let popup_width = 70u16.min(area.width.saturating_sub(2));
    let popup_height = 18u16.min(area.height.saturating_sub(2));
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

    // Title
    let title_line = Line::from(Span::styled(
        title,
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    ));
    Paragraph::new(title_line).render(
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
        buf,
    );

    // Separator
    let sep: String = "\u{2500}".repeat(inner.width as usize);
    buf.set_string(
        inner.x,
        inner.y + 1,
        &sep,
        Style::default().fg(theme.border_subtle),
    );

    // Fields start at y+2
    let fields_y = inner.y + 2;
    let label_width = 13u16;
    let value_width = inner.width.saturating_sub(label_width + 2);
    let label_style = Style::default().fg(theme.text_muted);

    let fields: Vec<(&str, String, bool)> = vec![
        ("Name:", edit.name.clone(), true),
        ("Trigger:", format_toggle_value(&edit.trigger), false),
        ("Prompt:", edit.prompt.clone(), true),
        ("Target:", format_toggle_value(&edit.target_mode), false),
        ("Cron:", edit.cron_expr.clone(), true),
        (
            "Enabled:",
            if edit.enabled { "yes" } else { "no" }.to_string(),
            false,
        ),
    ];

    for (i, (label, value, is_text)) in fields.iter().enumerate() {
        let row = fields_y + i as u16;
        if row >= inner.y + inner.height.saturating_sub(2) {
            break;
        }
        let is_focused = i == edit.focused_field;

        // Label
        buf.set_string(inner.x + 1, row, label, label_style);

        // Value
        let val_x = inner.x + 1 + label_width;
        if is_focused {
            // Highlight the focused field
            let bg = theme.primary;
            let fg = theme.background;
            let style = Style::default().fg(fg).bg(bg);

            if *is_text {
                // Text field with cursor
                let display = format!("{}_", value);
                let max_w = value_width as usize;
                let truncated = if display.chars().count() > max_w {
                    let skip = display.chars().count() - max_w;
                    display.chars().skip(skip).collect::<String>()
                } else {
                    display.clone()
                };
                // Clear the value area
                let blank: String = " ".repeat(value_width as usize);
                buf.set_string(val_x, row, &blank, style);
                buf.set_string(val_x, row, &truncated, style);
            } else {
                // Toggle field with arrows
                let display = format!("< {} >", value);
                let blank: String = " ".repeat(value_width as usize);
                buf.set_string(val_x, row, &blank, style);
                buf.set_string(val_x, row, &display, style);
            }
        } else {
            let style = Style::default().fg(theme.text);
            if *is_text {
                let max_w = value_width as usize;
                let display: String = if value.chars().count() > max_w {
                    let truncated: String = value.chars().take(max_w.saturating_sub(1)).collect();
                    format!("{}\u{2026}", truncated)
                } else {
                    value.clone()
                };
                buf.set_string(val_x, row, &display, style);
            } else {
                buf.set_string(val_x, row, value.as_str(), style);
            }
        }
    }

    // Separator before hint
    let hint_y = inner.y + inner.height.saturating_sub(2);
    if hint_y > fields_y + fields.len() as u16 {
        buf.set_string(
            inner.x,
            hint_y,
            &sep,
            Style::default().fg(theme.border_subtle),
        );
    }

    // Hint line
    let hint_row = inner.y + inner.height.saturating_sub(1);
    let hints = vec![
        ("Tab", "next"),
        ("S-Tab", "prev"),
        ("Enter/^S", "save"),
        ("Esc", "cancel"),
    ];
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
    Paragraph::new(Line::from(spans)).render(
        Rect {
            x: inner.x,
            y: hint_row,
            width: inner.width,
            height: 1,
        },
        buf,
    );
}

/// Format a toggle field value for display.
fn format_toggle_value(val: &str) -> String {
    match val {
        "manual" => "manual".to_string(),
        "scheduled" => "scheduled".to_string(),
        "on_session_idle" => "on session idle".to_string(),
        "daily_summary" => "daily summary".to_string(),
        "new_session" => "new session".to_string(),
        "existing_session" => "existing session".to_string(),
        other => other.to_string(),
    }
}

/// Render a delete confirmation overlay centered on the popup area.
fn render_delete_confirmation(
    state: &crate::app::RoutinePanelState,
    parent_area: Rect,
    buf: &mut Buffer,
    theme: &ThemeColors,
) {
    let routine_id = match &state.confirm_delete {
        Some(id) => id,
        None => return,
    };

    let routine_name = state
        .routines
        .iter()
        .find(|r| r.id == *routine_id)
        .map(|r| r.name.as_str())
        .unwrap_or("unknown");

    let prompt = format!("Delete routine '{}'? (y/n)", routine_name);
    let dialog_width = (prompt.len() as u16 + 6).min(parent_area.width.saturating_sub(4));
    let dialog_height = 3u16;
    let dialog_x = parent_area.x + (parent_area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = parent_area.y + (parent_area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x: dialog_x,
        y: dialog_y,
        width: dialog_width,
        height: dialog_height,
    };

    // Draw dialog background
    Clear.render(dialog_area, buf);
    let block = Block::default().style(Style::default().bg(theme.background_panel));
    block.render(dialog_area, buf);

    // Draw border (simple box)
    let border_style = Style::default().fg(Color::Red).bg(theme.background_panel);
    let top_border = format!(
        "\u{250c}{}\u{2510}",
        "\u{2500}".repeat((dialog_width as usize).saturating_sub(2))
    );
    let bottom_border = format!(
        "\u{2514}{}\u{2518}",
        "\u{2500}".repeat((dialog_width as usize).saturating_sub(2))
    );
    buf.set_string(dialog_x, dialog_y, &top_border, border_style);
    buf.set_string(
        dialog_x,
        dialog_y + dialog_height - 1,
        &bottom_border,
        border_style,
    );
    // Side borders
    buf.set_string(dialog_x, dialog_y + 1, "\u{2502}", border_style);
    buf.set_string(
        dialog_x + dialog_width - 1,
        dialog_y + 1,
        "\u{2502}",
        border_style,
    );

    // Clear the inner line and write the prompt
    let inner_width = dialog_width.saturating_sub(2);
    let blank: String = " ".repeat(inner_width as usize);
    buf.set_string(
        dialog_x + 1,
        dialog_y + 1,
        &blank,
        Style::default().bg(theme.background_panel),
    );

    // Center the prompt text
    let prompt_x = dialog_x + 1 + (inner_width.saturating_sub(prompt.len() as u16)) / 2;
    let prompt_style = Style::default()
        .fg(Color::Red)
        .bg(theme.background_panel)
        .add_modifier(Modifier::BOLD);
    buf.set_string(prompt_x, dialog_y + 1, &prompt, prompt_style);
}

fn render_detail_pane(
    buf: &mut Buffer,
    area: Rect,
    state: &crate::app::RoutinePanelState,
    theme: &ThemeColors,
) {
    let routine = match state.routines.get(state.selected) {
        Some(r) => r,
        None => return,
    };

    let label_style = Style::default().fg(theme.text_muted);
    let value_style = Style::default().fg(theme.text);
    let error_style = Style::default().fg(Color::Red);
    let w = area.width as usize;

    let mut lines: Vec<Line> = Vec::new();

    // Trigger & Action
    lines.push(Line::from(vec![
        Span::styled(" Trigger: ", label_style),
        Span::styled(&routine.trigger, value_style),
        Span::styled("  Action: ", label_style),
        Span::styled(&routine.action, value_style),
    ]));

    // Cron / Schedule
    if let Some(ref cron) = routine.cron_expr {
        lines.push(Line::from(vec![
            Span::styled(" Cron:    ", label_style),
            Span::styled(cron.as_str(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Target
    let target = match routine.target_mode.as_deref() {
        Some("new_session") => "New session".to_string(),
        Some("existing_session") => {
            if let Some(ref sid) = routine.session_id {
                let short: String = sid.chars().take(12).collect();
                format!("Session {short}...")
            } else {
                "Current session".to_string()
            }
        }
        _ => "\u{2014}".to_string(),
    };
    lines.push(Line::from(vec![
        Span::styled(" Target:  ", label_style),
        Span::styled(target, value_style),
    ]));

    // Provider/Model
    if routine.provider_id.is_some() || routine.model_id.is_some() {
        let model_str = format!(
            "{}/{}",
            routine.provider_id.as_deref().unwrap_or("default"),
            routine.model_id.as_deref().unwrap_or("default"),
        );
        lines.push(Line::from(vec![
            Span::styled(" Model:   ", label_style),
            Span::styled(model_str, value_style),
        ]));
    }

    // Prompt (truncated)
    if let Some(ref prompt) = routine.prompt {
        let max_prompt = w.saturating_sub(12);
        let display: String = if prompt.chars().count() > max_prompt {
            let truncated: String = prompt.chars().take(max_prompt.saturating_sub(3)).collect();
            format!("{truncated}...")
        } else {
            prompt.clone()
        };
        // Replace newlines with spaces for single-line display
        let display = display.replace('\n', " ");
        lines.push(Line::from(vec![
            Span::styled(" Prompt:  ", label_style),
            Span::styled(display, value_style),
        ]));
    }

    // Timing
    let last = routine.last_run_at.as_deref().unwrap_or("never");
    let next = routine.next_run_at.as_deref().unwrap_or("\u{2014}");
    lines.push(Line::from(vec![
        Span::styled(" Last run: ", label_style),
        Span::styled(format_compact_time(last), value_style),
        Span::styled("  Next: ", label_style),
        Span::styled(format_compact_time(next), value_style),
    ]));

    // Error
    if let Some(ref err) = routine.last_error {
        let max_err = w.saturating_sub(12);
        let display: String = if err.chars().count() > max_err {
            let truncated: String = err.chars().take(max_err.saturating_sub(3)).collect();
            format!("{truncated}...")
        } else {
            err.clone()
        };
        lines.push(Line::from(vec![
            Span::styled(" Error:   ", label_style),
            Span::styled(display, error_style),
        ]));
    }

    for (i, line) in lines.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }
        Paragraph::new(line.clone()).render(
            Rect {
                x: area.x,
                y: area.y + i as u16,
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

/// Format an ISO timestamp or short string compactly for TUI display.
fn format_compact_time(ts: &str) -> String {
    if ts == "never" || ts == "\u{2014}" {
        return ts.to_string();
    }
    // Try to extract HH:MM from ISO
    if let Some(t_idx) = ts.find('T') {
        let time_part = &ts[t_idx + 1..];
        let hm: String = time_part.chars().take(5).collect();
        return hm;
    }
    if ts.len() > 16 {
        ts[..16].to_string()
    } else {
        ts.to_string()
    }
}

fn render_status_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &crate::app::RoutinePanelState,
    theme: &ThemeColors,
) {
    let total = state.routines.len();
    let enabled = state.routines.iter().filter(|r| r.enabled).count();
    let scheduled = state
        .routines
        .iter()
        .filter(|r| r.trigger == "scheduled")
        .count();

    let mut spans = vec![
        Span::styled(format!(" {} total", total), Style::default().fg(theme.text)),
        Span::styled("  \u{25cf} ", Style::default().fg(Color::Green)),
        Span::styled(
            format!("{} enabled", enabled),
            Style::default().fg(theme.text_muted),
        ),
    ];
    if scheduled > 0 {
        spans.push(Span::styled(
            "  \u{23f0} ",
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled(
            format!("{} scheduled", scheduled),
            Style::default().fg(theme.text_muted),
        ));
    }
    if let Some(ref _running_id) = state.running {
        spans.push(Span::styled(
            "  \u{25b6} running",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let line = Line::from(spans);
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

fn render_hint_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    _state: &crate::app::RoutinePanelState,
    theme: &ThemeColors,
) {
    let hints = vec![
        ("Enter", "run"),
        ("n", "new"),
        ("E", "edit"),
        ("x", "delete"),
        ("d", "detail"),
        ("e", "enable"),
        ("r", "refresh"),
        ("q", "close"),
    ];

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
