use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::app::{App, SidebarItem};
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;

/// Handle keys when the sidebar is focused.
pub(super) fn handle_sidebar_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        crossterm::event::KeyCode::Char('g') => {
            if app.sidebar_pending_g {
                app.sidebar_cursor = 0;
                app.sidebar_pending_g = false;
            } else {
                app.sidebar_pending_g = true;
            }
        }
        crossterm::event::KeyCode::Char('G') => {
            let max = app.sidebar_item_count().saturating_sub(1);
            app.sidebar_cursor = max;
            app.sidebar_pending_g = false;
        }
        crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
            if app.sidebar_cursor > 0 {
                app.sidebar_cursor -= 1;
            }
            app.sidebar_pending_g = false;
        }
        crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
            let max = app.sidebar_item_count().saturating_sub(1);
            if app.sidebar_cursor < max {
                app.sidebar_cursor += 1;
            }
            app.sidebar_pending_g = false;
        }
        crossterm::event::KeyCode::Enter => {
            app.sidebar_pending_g = false;
            match app.sidebar_item_at(app.sidebar_cursor) {
                Some(SidebarItem::Project(idx)) => {
                    if app.active_project != idx {
                        app.switch_project(idx);
                    }
                    if app.sessions_expanded_for == Some(idx) {
                        app.sessions_expanded_for = None;
                    } else {
                        app.sessions_expanded_for = Some(idx);
                    }
                }
                Some(SidebarItem::NewSession(proj_idx)) => {
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
                Some(SidebarItem::Session(proj_idx, session_id)) => {
                    if app.active_project != proj_idx {
                        app.switch_project(proj_idx);
                    }
                    if let Some(project) = app.projects.get(proj_idx) {
                        if project.ptys.contains_key(&session_id) {
                            app.projects[proj_idx].active_session = Some(session_id.clone());
                            app.unseen_sessions.remove(&session_id);
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
                    app.layout.focused = PanelId::TerminalPane;
                }
                Some(SidebarItem::SubAgentSession(proj_idx, session_id)) => {
                    if app.active_project != proj_idx {
                        app.switch_project(proj_idx);
                    }
                    if let Some(project) = app.projects.get(proj_idx) {
                        if project.ptys.contains_key(&session_id) {
                            app.projects[proj_idx].active_session = Some(session_id.clone());
                            app.unseen_sessions.remove(&session_id);
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
                    app.layout.focused = PanelId::TerminalPane;
                }
                Some(SidebarItem::MoreSessions(proj_idx)) => {
                    if app.active_project != proj_idx {
                        app.switch_project(proj_idx);
                    }
                    app.open_session_search();
                }
                Some(SidebarItem::AddProject) => {
                    app.start_add_project();
                }
                None => {}
            }
        }
        crossterm::event::KeyCode::Char('a') => {
            app.sidebar_pending_g = false;
            app.start_add_project();
        }
        crossterm::event::KeyCode::Char('o') => {
            app.sidebar_pending_g = false;
            if let Some(SidebarItem::Session(_proj_idx, ref session_id)) =
                app.sidebar_item_at(app.sidebar_cursor)
            {
                let sid = session_id.clone();
                if app.subagents_expanded_for.as_deref() == Some(&sid) {
                    app.subagents_expanded_for = None;
                } else {
                    app.subagents_expanded_for = Some(sid);
                }
            }
        }
        crossterm::event::KeyCode::Char('r') => {
            app.sidebar_pending_g = false;
            match app.sidebar_item_at(app.sidebar_cursor) {
                Some(SidebarItem::Session(proj_idx, session_id))
                | Some(SidebarItem::SubAgentSession(proj_idx, session_id)) => {
                    if app.active_project != proj_idx {
                        app.switch_project(proj_idx);
                    }
                    if let Some(project) = app.projects.get(proj_idx) {
                        if project.ptys.contains_key(&session_id) {
                            app.projects[proj_idx].active_session = Some(session_id.clone());
                            app.unseen_sessions.remove(&session_id);
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
                    app.layout.focused = PanelId::TerminalPane;
                }
                _ => {}
            }
        }
        crossterm::event::KeyCode::Char('d') => {
            app.sidebar_pending_g = false;
            if let Some(SidebarItem::Project(idx)) = app.sidebar_item_at(app.sidebar_cursor) {
                app.confirm_delete = Some(idx);
            }
        }
        crossterm::event::KeyCode::Char('?') => {
            app.sidebar_pending_g = false;
            app.toggle_cheatsheet();
        }
        crossterm::event::KeyCode::Char('q') => {
            app.sidebar_pending_g = false;
            app.should_quit = true;
        }
        crossterm::event::KeyCode::Esc => {
            app.sidebar_pending_g = false;
            app.vim_mode = VimMode::Normal;
        }
        _ => {
            app.sidebar_pending_g = false;
        }
    }
    Ok(())
}
