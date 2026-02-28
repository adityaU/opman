use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::{App, WatcherField};
use crate::theme::ThemeColors;

pub struct WatcherModal<'a> {
    app: &'a App,
}

impl<'a> WatcherModal<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app }
    }

    pub fn render_popup(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.app.theme;
        let state = match &self.app.watcher_modal {
            Some(s) => s,
            None => return,
        };

        // 70% width, 70% height, centered
        let popup_width = (area.width * 70 / 100)
            .max(60)
            .min(area.width.saturating_sub(2));
        let popup_height = (area.height * 70 / 100)
            .max(14)
            .min(area.height.saturating_sub(2));

        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        super::render_overlay_dim(area, buf);
        Clear.render(popup_area, buf);

        let block = Block::default().style(Style::default().bg(theme.background_panel));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        if inner.height < 6 || inner.width < 20 {
            return;
        }

        let content_x = inner.x + 1;
        let content_width = inner.width.saturating_sub(2);

        // Title bar
        let title_line = Line::from(vec![
            Span::styled(
                " Session Watcher ",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " monitor idle sessions",
                Style::default().fg(theme.text_muted),
            ),
        ]);
        let esc_hint = "esc";
        buf.set_line(content_x, inner.y + 1, &title_line, content_width);
        buf.set_string(
            content_x + content_width.saturating_sub(esc_hint.len() as u16),
            inner.y + 1,
            esc_hint,
            Style::default().fg(theme.text_muted),
        );

        // Separator below title
        let sep: String = "─".repeat(content_width as usize);
        buf.set_string(
            content_x,
            inner.y + 2,
            &sep,
            Style::default().fg(theme.border_subtle),
        );

        // Two-panel layout: left 30%, right 70%
        let body_y = inner.y + 3;
        let body_height = inner.height.saturating_sub(5); // title(2) + sep(1) + hint(1) + pad(1)
        let left_width = (content_width * 30 / 100).max(15);
        let right_width = content_width.saturating_sub(left_width + 1); // 1 for separator

        let left_area = Rect::new(content_x, body_y, left_width, body_height);
        let sep_x = content_x + left_width;
        let right_area = Rect::new(sep_x + 1, body_y, right_width, body_height);

        // Vertical separator
        for row in body_y..body_y + body_height {
            buf.set_string(
                sep_x,
                row,
                "│",
                Style::default().fg(theme.border_subtle),
            );
        }

        // Render left panel (session list)
        render_session_list(buf, left_area, state, theme);

        // Render right panel (watcher config)
        render_config_panel(buf, right_area, state, theme);

        // Hint bar at bottom
        let hint_y = inner.y + inner.height.saturating_sub(1);
        render_hint_bar(buf, content_x, hint_y, content_width, state, theme);
    }
}

fn render_session_list(
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

fn render_config_panel(
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
        buf.set_string(cx, y, "Select a session", Style::default().fg(theme.text_muted));
        return;
    }

    let selected_entry = &state.sessions[state.selected_session_idx.min(state.sessions.len().saturating_sub(1))];

    // Session info header
    let info = format!("{} / {}", selected_entry.project_name, selected_entry.title);
    let info_display = if info.len() > cw {
        format!("{}...", &info[..cw.saturating_sub(3).min(info.len())])
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
    let msg_rows = 4u16.min(area.y + area.height - y - 6); // leave room for other fields
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
        if state.message_lines.is_empty() || (state.message_lines.len() == 1 && state.message_lines[0].is_empty()) {
            buf.set_string(cx, y, "(empty - type a message)", Style::default().fg(theme.text_muted));
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
        Style::default()
            .fg(theme.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    buf.set_string(cx, y, check_char, check_style);
    buf.set_string(cx + 4, y, "Include original message", label_style);
    y += 1;

    // ── Original Message List (only if toggled on) ──
    if state.include_original {
        if y >= area.y + area.height - 2 {
            return;
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
                    format!("{}...", &preview[..cw.saturating_sub(5).min(preview.len())])
                } else {
                    preview
                };
                buf.set_string(cx + 1, y, &preview, row_style);
                y += 1;
            }
        }
    }

    // ── Timeout Input ──
    if y >= area.y + area.height - 1 {
        return;
    }
    y += if state.include_original { 0 } else { 1 }; // spacing
    if y >= area.y + area.height - 1 {
        return;
    }
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
}

fn render_multiline_editor(
    buf: &mut Buffer,
    cx: u16,
    y: u16,
    cw: usize,
    height: usize,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    // Build visual rows with wrapping (same approach as context_input.rs)
    let mut vrows: Vec<(usize, usize, usize)> = Vec::new();
    let mut cursor_vrow: usize = 0;

    for (li, line_text) in state.message_lines.iter().enumerate() {
        if cw == 0 {
            vrows.push((li, 0, line_text.len()));
            continue;
        }
        if line_text.is_empty() {
            if li == state.message_cursor_row {
                cursor_vrow = vrows.len();
            }
            vrows.push((li, 0, 0));
            continue;
        }
        let mut pos = 0;
        while pos < line_text.len() {
            let chunk_start = pos;
            let mut col = 0;
            let mut end = pos;
            for ch in line_text[pos..].chars() {
                if col + 1 > cw {
                    break;
                }
                col += 1;
                end += ch.len_utf8();
            }
            if end == pos {
                let ch = line_text[pos..].chars().next().unwrap();
                end += ch.len_utf8();
            }
            if li == state.message_cursor_row
                && state.message_cursor_col >= chunk_start
                && (state.message_cursor_col < end
                    || (state.message_cursor_col == end && end == line_text.len()))
            {
                cursor_vrow = vrows.len();
            }
            vrows.push((li, chunk_start, end));
            pos = end;
        }
    }

    // Scroll so cursor is visible
    let visible_start = if cursor_vrow >= height {
        cursor_vrow - height + 1
    } else {
        0
    };

    for vi in 0..height {
        let vrow_idx = visible_start + vi;
        let row_y = y + vi as u16;
        if vrow_idx >= vrows.len() {
            break;
        }

        let (li, byte_start, byte_end) = vrows[vrow_idx];
        let chunk = &state.message_lines[li][byte_start..byte_end];
        let is_cursor_vrow = vrow_idx == cursor_vrow;

        if is_cursor_vrow {
            let cursor_byte = state
                .message_cursor_col
                .min(byte_end)
                .saturating_sub(byte_start);
            let before = &chunk[..cursor_byte.min(chunk.len())];
            let (cursor_ch, cursor_ch_len) = if cursor_byte < chunk.len() {
                let ch = chunk[cursor_byte..].chars().next().unwrap();
                (
                    &chunk[cursor_byte..cursor_byte + ch.len_utf8()],
                    ch.len_utf8(),
                )
            } else {
                (" ", 0)
            };
            let after = if cursor_ch_len > 0 && cursor_byte + cursor_ch_len <= chunk.len() {
                &chunk[cursor_byte + cursor_ch_len..]
            } else {
                ""
            };

            let spans = vec![
                Span::styled(before, Style::default().fg(theme.text)),
                Span::styled(
                    cursor_ch,
                    Style::default()
                        .bg(theme.text)
                        .fg(theme.background)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(after, Style::default().fg(theme.text)),
            ];
            let line = Line::from(spans);
            Paragraph::new(line).render(Rect::new(cx, row_y, cw as u16, 1), buf);
        } else {
            buf.set_string(cx, row_y, chunk, Style::default().fg(theme.text));
        }
    }
}

fn render_hint_bar(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    state: &crate::app::WatcherModalState,
    theme: &ThemeColors,
) {
    let mut spans = vec![
        Span::styled("  Tab", Style::default().fg(theme.accent)),
        Span::styled(" next  ", Style::default().fg(theme.text_muted)),
        Span::styled("S-Tab", Style::default().fg(theme.accent)),
        Span::styled(" prev  ", Style::default().fg(theme.text_muted)),
    ];

    match state.active_field {
        WatcherField::SessionList => {
            spans.extend(vec![
                Span::styled("↑↓", Style::default().fg(theme.accent)),
                Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
                Span::styled("d", Style::default().fg(theme.accent)),
                Span::styled(" remove  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::Message => {
            spans.extend(vec![
                Span::styled("Enter", Style::default().fg(theme.accent)),
                Span::styled(" newline  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::IncludeOriginal => {
            spans.extend(vec![
                Span::styled("Space", Style::default().fg(theme.accent)),
                Span::styled(" toggle  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::OriginalMessageList => {
            spans.extend(vec![
                Span::styled("↑↓", Style::default().fg(theme.accent)),
                Span::styled(" navigate  ", Style::default().fg(theme.text_muted)),
            ]);
        }
        WatcherField::TimeoutInput => {}
    }

    spans.extend(vec![
        Span::styled("Ctrl+D", Style::default().fg(theme.success)),
        Span::styled(" submit  ", Style::default().fg(theme.text_muted)),
        Span::styled("Esc", Style::default().fg(theme.warning)),
        Span::styled(" close", Style::default().fg(theme.text_muted)),
    ]);

    let line = Line::from(spans);
    Paragraph::new(line).render(Rect::new(x, y, width, 1), buf);
}
