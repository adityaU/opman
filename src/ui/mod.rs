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
mod render_helpers;
mod render_overlays;
pub mod session_selector;
pub mod sidebar;
pub mod slack_log_panel;
pub mod status_bar;
pub mod submodule_popup;
pub mod tag_popup;
pub mod term_render;
pub mod terminal_pane;
pub mod todo_panel;
pub mod watcher_modal;

pub use render_helpers::render_overlay_dim;
#[allow(unused_imports)]
pub use render_helpers::render_pane_title_bar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use ratatui::Frame;

use crate::app::App;

use self::gitui_pane::GituiPane;
use self::integrated_terminal::IntegratedTerminal;
use self::layout_manager::PanelId;
use self::neovim_pane::NeovimPane;
use self::sidebar::Sidebar;
use self::status_bar::StatusBar;
use self::terminal_pane::TerminalPane;

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
                    render_helpers::dim_panel(rect, frame.buffer_mut(), 1.0 - pct / 100.0);
                }
            }
        }

        let seps: Vec<_> = app.layout.get_separator_rects().to_vec();
        for sep in &seps {
            let is_vertical = sep.direction == layout_manager::SplitDirection::Horizontal;
            render_helpers::render_separator(
                frame,
                app,
                sep.rect,
                is_vertical,
                app.theme.border_subtle,
            );
        }
        render_helpers::render_separator_junctions(frame, app, &seps);
    }

    render_status_bar(frame, app, status_area);
    render_overlays::render_overlays(frame, app, size);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_bar = StatusBar::new(app);
    frame.render_widget(status_bar, area);
}
