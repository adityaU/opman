use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use ratatui::Frame;

use crate::app::{App, InputMode};
use crate::command_palette::CommandPalette;
use crate::theme::ThemeColors;
use crate::vim_mode::VimMode;
use crate::which_key::WhichKeyState;

use super::cheatsheet::CheatSheet;
use super::config_panel::ConfigPanel;
use super::fuzzy_picker::FuzzyPicker;
use super::render_helpers::render_overlay_dim;
use super::sidebar::SessionSearchPanel;

pub(super) fn render_overlays(frame: &mut Frame, app: &App, size: Rect) {
    if app.input_mode == InputMode::FuzzyPicker {
        if let Some(ref _picker) = app.fuzzy_picker {
            let fuzzy = FuzzyPicker::new(app);
            fuzzy.render_popup(size, frame.buffer_mut());
        }
    }

    if app.input_mode == InputMode::AddProject {
        let dialog = super::input_dialog::InputDialog::new(app);
        dialog.render_popup(size, frame.buffer_mut());
    }

    if app.session_search_mode {
        let panel = SessionSearchPanel::new(app);
        panel.render_popup(size, frame.buffer_mut());
    }

    if app.vim_mode == VimMode::Command {
        render_command_palette(frame, &app.command_palette, &app.theme, size);
    }

    if app.vim_mode == VimMode::WhichKey {
        render_which_key(frame, &app.which_key, &app.theme, size);
    }

    if app.show_cheatsheet {
        let cheatsheet = CheatSheet::new(&app.theme, &app.runtime_keymap);
        cheatsheet.render_popup(size, frame.buffer_mut());
    }

    if app.show_config_panel {
        let panel = ConfigPanel::new(
            &app.theme,
            app.config_panel_selected,
            app.config.settings.follow_edits_in_neovim,
            app.config.settings.unfocused_dim_percent,
            app.config.settings.slack.enabled,
            app.config.settings.slack.relay_buffer_secs,
        );
        panel.render_popup(size, frame.buffer_mut());
    }

    if app.show_slack_log {
        if let Some(ref slack_state_arc) = app.slack_state {
            if let Ok(state) = slack_state_arc.try_lock() {
                let panel = super::slack_log_panel::SlackLogPanel::new(
                    &app.theme,
                    &state.event_log,
                    &state.metrics,
                    app.slack_log_scroll,
                );
                panel.render_popup(size, frame.buffer_mut());
            }
        }
    }
    if app.session_selector.is_some() {
        super::session_selector::render_session_selector(app, size, frame.buffer_mut());
    }

    if app.todo_panel.is_some() {
        super::todo_panel::render_todo_panel(app, size, frame.buffer_mut());
    }

    if app.context_input.is_some() {
        let ci = super::context_input::ContextInput::new(app);
        ci.render_popup(size, frame.buffer_mut());
    }

    if app.watcher_modal.is_some() {
        let wm = super::watcher_modal::WatcherModal::new(app);
        wm.render_popup(size, frame.buffer_mut());
    }
}

fn render_command_palette(
    frame: &mut Frame,
    palette: &CommandPalette,
    theme: &ThemeColors,
    area: Rect,
) {
    let filtered = palette.filtered_commands();
    let result_count = filtered.len().min(8) as u16;
    let popup_height = result_count + 5; // title + input + separator + results + padding
    let popup_width = (area.width * 50 / 100)
        .max(40)
        .min(area.width.saturating_sub(2));
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height.min(area.height.saturating_sub(popup_y.saturating_sub(area.y))),
    };

    let buf = frame.buffer_mut();
    render_overlay_dim(area, buf);
    Clear.render(popup_area, buf);

    let block = Block::default().style(Style::default().bg(theme.background_panel));
    let panel_inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let inner_x = panel_inner.x + 2;
    let inner_width = panel_inner.width.saturating_sub(4);
    let mut y = panel_inner.y + 1;

    if panel_inner.height < 4 {
        return;
    }

    let title_span = Span::styled(
        "Commands",
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let esc_span = Span::styled("esc", Style::default().fg(theme.text_muted));
    let fill_len = inner_width as usize - "Commands".len() - "esc".len();
    let fill = " ".repeat(fill_len.max(1));
    let title_line = Line::from(vec![title_span, Span::raw(fill), esc_span]);
    buf.set_line(inner_x, y, &title_line, inner_width);
    y += 1;

    let prompt = ":";
    buf.set_string(
        inner_x,
        y,
        prompt,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    buf.set_string(
        inner_x + 1,
        y,
        &palette.query,
        Style::default().fg(theme.text),
    );
    let cursor_x = inner_x + 1 + palette.cursor_pos as u16;
    if cursor_x < inner_x + inner_width {
        let cursor_char = palette.query[palette.cursor_pos..]
            .chars()
            .next()
            .unwrap_or(' ');
        buf.set_string(
            cursor_x,
            y,
            cursor_char.to_string(),
            Style::default().fg(theme.background).bg(theme.text),
        );
    }
    y += 1;

    let sep: String = "─".repeat(inner_width as usize);
    buf.set_string(inner_x, y, &sep, Style::default().fg(theme.border_subtle));
    y += 1;

    for (i, cmd) in filtered.iter().take(8).enumerate() {
        let row = y + i as u16;
        if row >= popup_area.y + popup_area.height {
            break;
        }
        let is_selected = i == palette.selected;
        let style = if is_selected {
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let blank: String = " ".repeat(inner_width as usize);
        buf.set_string(inner_x, row, &blank, style);

        let indicator = if is_selected { "> " } else { "  " };
        buf.set_string(inner_x, row, indicator, style);
        buf.set_string(inner_x + 2, row, &cmd.name, style);

        let hint_width = cmd.keys_hint.len() as u16;
        if inner_width > hint_width + 4 {
            let hint_x = inner_x + inner_width - hint_width - 1;
            let hint_style = if is_selected {
                style
            } else {
                Style::default().fg(theme.text_muted)
            };
            buf.set_string(hint_x, row, &cmd.keys_hint, hint_style);
        }
    }
}

fn render_which_key(frame: &mut Frame, state: &WhichKeyState, theme: &ThemeColors, area: Rect) {
    use crate::which_key::format_key_label;

    let bindings = state.current_bindings();
    if bindings.is_empty() {
        return;
    }

    let row_count = bindings.len() as u16;
    let popup_height = row_count + 2; // title + rows + bottom padding

    let key_labels: Vec<String> = bindings.iter().map(|b| format_key_label(&b.key)).collect();

    let popup_width = 30u16
        .max(
            bindings
                .iter()
                .zip(key_labels.iter())
                .map(|(b, kl)| kl.len() as u16 + b.label.len() as u16 + 8)
                .max()
                .unwrap_or(20)
                + 4,
        )
        .min(area.width);

    let popup_x = area.x + area.width.saturating_sub(popup_width + 1);
    let popup_y = area.y + area.height.saturating_sub(popup_height + 1);

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    let buf = frame.buffer_mut();
    render_overlay_dim(area, buf);
    Clear.render(popup_area, buf);

    let block = Block::default().style(Style::default().bg(theme.background_panel));
    let block_inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let inner_x = block_inner.x + 2;
    let inner_width = block_inner.width.saturating_sub(4);

    let title_text = if state.pending_prefix_label.is_empty() {
        "Keys".to_string()
    } else {
        format!("{}+", state.pending_prefix_label)
    };
    let title_span = Span::styled(
        title_text.clone(),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    );
    let title_line = Line::from(vec![title_span]);
    buf.set_line(inner_x, block_inner.y + 1, &title_line, inner_width);

    let content_y = block_inner.y + 2;
    for (i, (binding, key_label)) in bindings.iter().zip(key_labels.iter()).enumerate() {
        let row = content_y + i as u16;
        if row >= block_inner.y + block_inner.height {
            break;
        }
        let key_span = Span::styled(
            format!("  {}  ", key_label),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );
        let arrow_span = Span::styled("→  ", Style::default().fg(theme.text_muted));
        let label_span = Span::styled(binding.label.clone(), Style::default().fg(theme.text));

        let line = Line::from(vec![key_span, arrow_span, label_span]);
        let paragraph = Paragraph::new(line);
        let row_area = Rect {
            x: inner_x,
            y: row,
            width: inner_width,
            height: 1,
        };
        paragraph.render(row_area, buf);
    }
}
