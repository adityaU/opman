pub mod blame_view;
pub mod branch_popup;
pub mod cheatsheet;
pub mod commit_popup;
pub mod config_panel;
pub mod context_input;
pub mod fuzzy_picker;
pub mod git_help_popup;
pub mod git_options_popup;
pub mod git_panel;
pub mod gitui_pane;
pub mod input_dialog;
pub mod integrated_terminal;
pub mod layout_manager;
pub mod neovim_pane;
pub mod remote_popup;
pub mod session_selector;
pub mod sidebar;
pub mod status_bar;
pub mod submodule_popup;
pub mod tag_popup;
pub mod term_render;
pub mod terminal_pane;
pub mod todo_panel;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use ratatui::Frame;

use crate::app::{App, InputMode};
use crate::command_palette::CommandPalette;
use crate::theme::ThemeColors;
use crate::vim_mode::VimMode;
use crate::which_key::WhichKeyState;

use self::cheatsheet::CheatSheet;
use self::config_panel::ConfigPanel;
use self::fuzzy_picker::FuzzyPicker;
use self::gitui_pane::GituiPane;
use self::integrated_terminal::IntegratedTerminal;
use self::layout_manager::PanelId;
use self::neovim_pane::NeovimPane;
use self::sidebar::{SessionSearchPanel, Sidebar};
use self::status_bar::StatusBar;
use self::terminal_pane::TerminalPane;

/// Must be called BEFORE rendering the popup panel on top.
pub fn render_overlay_dim(area: Rect, buf: &mut Buffer) {
    let dim_bg = Color::Rgb(10, 10, 10);
    let dim_fg = Color::Rgb(60, 60, 60);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(dim_bg);
                cell.set_fg(dim_fg);
            }
        }
    }
}

/// Dim an unfocused panel by reducing the brightness of every cell.
/// Blends each color channel toward a dark base by the given factor
/// (0.0 = fully dark, 1.0 = no change).  Driven by the
/// `unfocused_dim_percent` setting (0–100, default 20).
fn dim_panel(area: Rect, buf: &mut Buffer, factor: f32) {
    fn blend(c: Color, factor: f32) -> Color {
        match c {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as f32 * factor) as u8,
                (g as f32 * factor) as u8,
                (b as f32 * factor) as u8,
            ),
            Color::Reset => Color::Reset,
            other => other,
        }
    }
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_fg(blend(cell.fg, factor));
                cell.set_bg(blend(cell.bg, factor));
            }
        }
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    if app.layout.is_visible(PanelId::IntegratedTerminal) {
        app.ensure_shell_pty();
    }
    if app.layout.is_visible(PanelId::NeovimPane) {
        app.ensure_neovim_pty();
    }
    if app.layout.is_visible(PanelId::GitPanel) {
        app.ensure_gitui_pty();
    }

    let bg_block = Block::default().style(Style::default().bg(app.theme.background));
    Widget::render(bg_block, size, frame.buffer_mut());

    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(size);

    let content_area = status_chunks[0];
    let status_area = status_chunks[1];

    // Only recompute layout rects + resize PTYs when the terminal size
    // changed or the layout structure was modified (panels toggled, resized).
    if content_area != app.layout.last_area || app.layout.layout_dirty {
        app.layout.compute_rects(content_area);
        app.resize_all_ptys();
        app.layout.layout_dirty = false;
    }

    if app.popout_mode {
        let lines = vec![
            Line::from(vec![Span::styled(
                "  Panels Popped Out  ",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                format!(
                    "  {} external window{} active  ",
                    app.popout_windows.len(),
                    if app.popout_windows.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
                Style::default().fg(app.theme.text_muted),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Space+w+w ", Style::default().fg(app.theme.accent)),
                Span::styled("to restore  ", Style::default().fg(app.theme.text_muted)),
            ]),
        ];
        let text_height = lines.len() as u16;
        let text_width = 40u16;
        let popup_y = content_area.y + content_area.height.saturating_sub(text_height) / 2;
        let popup_x = content_area.x + content_area.width.saturating_sub(text_width) / 2;
        let popup_area = Rect::new(
            popup_x,
            popup_y,
            text_width.min(content_area.width),
            text_height.min(content_area.height),
        );
        let para = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(para, popup_area);
    } else if app.zen_mode {
        let focused = app.layout.focused;
        match focused {
            PanelId::Sidebar => {
                let sidebar = Sidebar::new(app);
                frame.render_widget(sidebar, content_area);
            }
            PanelId::TerminalPane => {
                let terminal_pane = TerminalPane::new(app);
                frame.render_widget(terminal_pane, content_area);
            }
            PanelId::NeovimPane => {
                let nvim = NeovimPane::new(app);
                frame.render_widget(nvim, content_area);
            }
            PanelId::IntegratedTerminal => {
                let integrated = IntegratedTerminal::new(app);
                frame.render_widget(integrated, content_area);
            }
            PanelId::GitPanel => {
                let gp = GituiPane::new(app);
                frame.render_widget(gp, content_area);
            }
        }
    } else {
        let focused = app.layout.focused;
        for panel_id in &[
            PanelId::Sidebar,
            PanelId::TerminalPane,
            PanelId::NeovimPane,
            PanelId::IntegratedTerminal,
            PanelId::GitPanel,
        ] {
            if !app.layout.is_visible(*panel_id) {
                continue;
            }
            let rect = match app.layout.panel_rect(*panel_id) {
                Some(r) => r,
                None => continue,
            };
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            match panel_id {
                PanelId::Sidebar => {
                    let sidebar = Sidebar::new(app);
                    frame.render_widget(sidebar, rect);
                }
                PanelId::TerminalPane => {
                    let terminal_pane = TerminalPane::new(app);
                    frame.render_widget(terminal_pane, rect);
                }
                PanelId::NeovimPane => {
                    let nvim = NeovimPane::new(app);
                    frame.render_widget(nvim, rect);
                }
                PanelId::IntegratedTerminal => {
                    let integrated = IntegratedTerminal::new(app);
                    frame.render_widget(integrated, rect);
                }
                PanelId::GitPanel => {
                    let gp = GituiPane::new(app);
                    frame.render_widget(gp, rect);
                }
            }
            // Dim unfocused panels so the focused one stands out.
            if *panel_id != focused {
                let pct = app.config.settings.unfocused_dim_percent.min(100) as f32;
                if pct > 0.0 {
                    dim_panel(rect, frame.buffer_mut(), 1.0 - pct / 100.0);
                }
            }
        }

        let seps: Vec<_> = app.layout.get_separator_rects().to_vec();
        for sep in &seps {
            let is_vertical = sep.direction == layout_manager::SplitDirection::Horizontal;
            render_separator(frame, app, sep.rect, is_vertical, app.theme.border_subtle);
        }
        render_separator_junctions(frame, app, &seps);
    }

    render_status_bar(frame, app, status_area);
    render_overlays(frame, app, size);
}

fn render_overlays(frame: &mut Frame, app: &App, size: Rect) {
    if app.input_mode == InputMode::FuzzyPicker {
        if let Some(ref _picker) = app.fuzzy_picker {
            let fuzzy = FuzzyPicker::new(app);
            fuzzy.render_popup(size, frame.buffer_mut());
        }
    }

    if app.input_mode == InputMode::AddProject {
        let dialog = input_dialog::InputDialog::new(app);
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
        );
        panel.render_popup(size, frame.buffer_mut());
    }

    if app.session_selector.is_some() {
        session_selector::render_session_selector(app, size, frame.buffer_mut());
    }

    if app.todo_panel.is_some() {
        todo_panel::render_todo_panel(app, size, frame.buffer_mut());
    }

    if app.context_input.is_some() {
        let ci = context_input::ContextInput::new(app);
        ci.render_popup(size, frame.buffer_mut());
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_bar = StatusBar::new(app);
    frame.render_widget(status_bar, area);
}

#[allow(dead_code)]
pub fn render_pane_title_bar(
    buf: &mut Buffer,
    area: Rect,
    label: &str,
    is_focused: bool,
    theme: &ThemeColors,
) {
    if area.width == 0 {
        return;
    }
    let title_style = if is_focused {
        Style::default()
            .fg(theme.primary)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    let sep_style = Style::default().fg(theme.border_subtle);
    let title_text = format!(" {} ", label);
    let title_len = title_text.len();
    let remaining = (area.width as usize).saturating_sub(title_len + 1);
    let left = "─";
    let right = "─".repeat(remaining);

    let line = Line::from(vec![
        Span::styled(left, sep_style),
        Span::styled(title_text, title_style),
        Span::styled(right, sep_style),
    ]);
    Paragraph::new(line).render(
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
        buf,
    );
}

fn render_separator(frame: &mut Frame, _app: &App, area: Rect, is_vertical: bool, color: Color) {
    let buf = frame.buffer_mut();
    let style = Style::default().fg(color);
    if is_vertical {
        for y in area.y..area.y + area.height {
            buf.set_string(area.x, y, "│", style);
        }
    } else {
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, "─", style);
        }
    }
}

fn render_separator_junctions(
    frame: &mut Frame,
    app: &App,
    seps: &[layout_manager::SeparatorRect],
) {
    let buf = frame.buffer_mut();
    let style = Style::default().fg(app.theme.border_subtle);

    for a in seps {
        for b in seps {
            let (v, h) = if a.direction == layout_manager::SplitDirection::Horizontal
                && b.direction == layout_manager::SplitDirection::Vertical
            {
                (a, b)
            } else {
                continue;
            };

            let vx = v.rect.x;
            let vy_start = v.rect.y;
            let vy_end = v.rect.y + v.rect.height;
            let hy = h.rect.y;
            let hx_start = h.rect.x;
            let hx_end = h.rect.x + h.rect.width;

            if vx >= hx_start && vx < hx_end {
                if hy == vy_end {
                    buf.set_string(vx, hy, "┴", style);
                } else if hy + 1 == vy_start {
                    buf.set_string(vx, hy, "┬", style);
                } else if hy >= vy_start && hy < vy_end {
                    buf.set_string(vx, hy, "┼", style);
                }
            }
        }
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
