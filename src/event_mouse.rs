use anyhow::Result;

use crate::app::{self, App};
use crate::input;
use crate::mouse_handler::{forward_mouse_to_pty, handle_integrated_terminal_mouse};
use crate::ui::layout_manager::PanelId;

/// Handle a mouse event inside the event loop.
pub(crate) fn handle_mouse_in_loop(
    app: &mut App,
    mouse_event: crossterm::event::MouseEvent,
) -> Result<()> {
    let (cols, rows) = crossterm::terminal::size()?;
    let area = ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));

    app.layout.compute_rects(area);
    let dragging = app.layout.handle_mouse(mouse_event, area);
    if dragging.is_some() {
        app.layout.compute_rects(area);
        input::resize_ptys(app);
    }

    // Forward mouse events to the PTY if the mouse is over a terminal panel
    if dragging.is_none() {
        // --- Status bar click-to-copy URL ---
        if mouse_event.row == rows.saturating_sub(1) {
            if let crossterm::event::MouseEventKind::Down(
                crossterm::event::MouseButton::Left,
            ) = mouse_event.kind
            {
                if let Some((start_x, end_x)) = app.status_bar_url_range.get() {
                    let col = mouse_event.column;
                    if col >= start_x && col < end_x {
                        let url = crate::app::base_url();
                        use std::io::Write as _;
                        use std::process::{Command, Stdio};
                        if let Ok(mut child) =
                            Command::new("pbcopy").stdin(Stdio::piped()).spawn()
                        {
                            if let Some(stdin) = child.stdin.as_mut() {
                                let _ = stdin.write_all(url.as_bytes());
                            }
                            let _ = child.wait();
                        }
                        app.toast_message = Some((
                            "Server URL copied!".to_string(),
                            std::time::Instant::now(),
                        ));
                        app.needs_redraw = true;
                    }
                }
            }
        }
        // In zen mode, only the focused panel is rendered full-screen,
        // but panel_at() still uses non-zen layout rects → wrong panel.
        // Short-circuit: always use the focused panel in zen mode.
        let panel_opt = if app.zen_mode {
            Some(app.layout.focused)
        } else {
            app.layout.panel_at(mouse_event.column, mouse_event.row)
        };
        if let Some(panel) = panel_opt {
            handle_mouse_on_panel(app, mouse_event, panel)?;
        }
    }

    Ok(())
}

/// Handle mouse event dispatched to a specific panel.
fn handle_mouse_on_panel(
    app: &mut App,
    mouse_event: crossterm::event::MouseEvent,
    panel: PanelId,
) -> Result<()> {
    match panel {
        PanelId::Sidebar => {
            handle_sidebar_click(app, mouse_event)?;
        }
        PanelId::TerminalPane => {
            if let Some(project) =
                app.projects.get_mut(app.active_project)
            {
                if let Some((pty, rect)) = project
                    .active_pty_mut()
                    .zip(app.layout.panel_rect(PanelId::TerminalPane))
                {
                    forward_mouse_to_pty(
                        pty,
                        &mouse_event,
                        rect.x,
                        rect.y,
                        PanelId::TerminalPane,
                        &mut app.terminal_selection,
                        &mut app.toast_message,
                    );
                }
            }
        }
        PanelId::IntegratedTerminal => {
            handle_integrated_terminal_mouse(app, mouse_event)?;
        }        PanelId::NeovimPane => {
            if let Some(project) =
                app.projects.get_mut(app.active_project)
            {
                if let Some((pty, rect)) = project
                    .active_resources_mut()
                    .and_then(|r| r.neovim_pty.as_mut())
                    .zip(app.layout.panel_rect(PanelId::NeovimPane))
                {
                    forward_mouse_to_pty(
                        pty,
                        &mouse_event,
                        rect.x,
                        rect.y,
                        PanelId::NeovimPane,
                        &mut app.terminal_selection,
                        &mut app.toast_message,
                    );
                }
            }
        }
        PanelId::GitPanel => {
            if let Some(project) =
                app.projects.get_mut(app.active_project)
            {
                if let Some((pty, rect)) = project
                    .gitui_pty
                    .as_mut()
                    .zip(app.layout.panel_rect(PanelId::GitPanel))
                {
                    forward_mouse_to_pty(
                        pty,
                        &mouse_event,
                        rect.x,
                        rect.y,
                        PanelId::GitPanel,
                        &mut app.terminal_selection,
                        &mut app.toast_message,
                    );
                }
            }
        }
    }
    Ok(())
}

/// Handle sidebar mouse click.
fn handle_sidebar_click(
    app: &mut App,
    mouse_event: crossterm::event::MouseEvent,
) -> Result<()> {
    if let crossterm::event::MouseEventKind::Down(
        crossterm::event::MouseButton::Left,
    ) = mouse_event.kind
    {
        if let Some(rect) = app.layout.panel_rect(PanelId::Sidebar) {
            let relative_y =
                mouse_event.row.saturating_sub(rect.y) as usize;
            let item_count = app.sidebar_item_count();
            if relative_y < item_count {
                app.sidebar_cursor = relative_y;
                app.layout.focused = PanelId::Sidebar;
                if let Some(item) = app.sidebar_item_at(relative_y) {
                    handle_sidebar_item_click(app, item, mouse_event, rect)?;
                }
            }
        }
    }
    Ok(())
}

/// Handle clicking on a specific sidebar item.
fn handle_sidebar_item_click(
    app: &mut App,
    item: app::SidebarItem,
    mouse_event: crossterm::event::MouseEvent,
    rect: ratatui::layout::Rect,
) -> Result<()> {
    match item {
        app::SidebarItem::Project(idx) => {
            if app.active_project != idx {
                app.switch_project(idx);
            }
            if app.sessions_expanded_for == Some(idx) {
                app.sessions_expanded_for = None;
            } else {
                app.sessions_expanded_for = Some(idx);
            }
        }
        app::SidebarItem::NewSession(proj_idx) => {
            if app.active_project != proj_idx {
                app.switch_project(proj_idx);
            }
            if let Some(project) = app.projects.get_mut(proj_idx) {
                if let Some(pty) = project.active_pty_mut() {
                    let _ = pty.kill();
                }
                if let Some(sid) = project.active_session.take() {
                    project.ptys.remove(&sid);
                }
            }
            app.pending_new_session = Some(proj_idx);
            app.pending_session_select = None;
            app.layout.focused = PanelId::TerminalPane;
        }
        app::SidebarItem::Session(proj_idx, session_id) => {
            if app.active_project != proj_idx {
                app.switch_project(proj_idx);
            }
            // Check if click is on the arrow (▶/▼) column to toggle subagents
            let relative_x = mouse_event.column.saturating_sub(rect.x) as usize;
            let has_subagents = !app.subagent_sessions(proj_idx, &session_id).is_empty();
            // Arrow sits at columns 6-9 ("    └ " = 6, optional "● " = +2, arrow "▶ " = 2)
            if has_subagents && relative_x >= 6 && relative_x <= 9 {
                if app.subagents_expanded_for.as_deref() == Some(&session_id) {
                    app.subagents_expanded_for = None;
                } else {
                    app.subagents_expanded_for = Some(session_id);
                }
            } else {
                select_session_or_pending(app, proj_idx, session_id);
                app.layout.focused = PanelId::TerminalPane;
            }
        }
        app::SidebarItem::MoreSessions(proj_idx) => {
            if app.active_project != proj_idx {
                app.switch_project(proj_idx);
            }
            app.open_session_search();
        }
        app::SidebarItem::SubAgentSession(proj_idx, session_id) => {
            if app.active_project != proj_idx {
                app.switch_project(proj_idx);
            }
            select_session_or_pending(app, proj_idx, session_id);
            app.layout.focused = PanelId::TerminalPane;
        }
        app::SidebarItem::AddProject => {
            app.start_add_project();
        }
    }
    Ok(())
}

/// Select a session if PTY exists, otherwise set pending_session_select.
fn select_session_or_pending(app: &mut App, proj_idx: usize, session_id: String) {
    if let Some(project) = app.projects.get(proj_idx) {
        if project.ptys.contains_key(&session_id) {
            app.projects[proj_idx].active_session = Some(session_id.clone());
            app.active_project = proj_idx;
            let dir = app.projects[proj_idx].path.to_string_lossy().to_string();
            let sid = session_id.clone();
            let base_url = crate::app::base_url().to_string();
            tokio::spawn(async move {
                let client = crate::api::ApiClient::new();
                let _ = client.select_session(&base_url, &dir, &sid).await;
            });
        } else {
            app.pending_session_select = Some((proj_idx, session_id));
        }
    }
}
