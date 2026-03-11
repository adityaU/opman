use anyhow::Result;

use crate::app::{App, BackgroundEvent};
use crate::command_palette::CommandAction;
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;

use super::resize_ptys;

pub(super) fn execute_command_action_ext(app: &mut App, action: CommandAction) -> Result<()> {
    match action {
        CommandAction::SlackConnect
        | CommandAction::SlackOAuth
        | CommandAction::SlackDisconnect
        | CommandAction::SlackStatus
        | CommandAction::SlackLogs => {
            return super::command_action_slack::handle_slack_action(app, action);
        }
        CommandAction::ConfigPanel => {
            app.show_config_panel = !app.show_config_panel;
            app.config_panel_selected = 0;
        }
        CommandAction::Quit => {
            app.should_quit = true;
        }
        CommandAction::InsertMode => {
            app.vim_mode = VimMode::Insert;
        }
        CommandAction::CommandMode => {
            app.vim_mode = VimMode::Command;
            app.command_palette.reset();
            app.command_palette.tick();
        }
        CommandAction::ResizeMode => {
            app.vim_mode = VimMode::Resize;
        }
        CommandAction::ResizeLeft => {
            app.layout.resize_focused(-1, 0);
            resize_ptys(app);
        }
        CommandAction::ResizeRight => {
            app.layout.resize_focused(1, 0);
            resize_ptys(app);
        }
        CommandAction::ResizeDown => {
            app.layout.resize_focused(0, 1);
            resize_ptys(app);
        }
        CommandAction::ResizeUp => {
            app.layout.resize_focused(0, -1);
            resize_ptys(app);
        }
        CommandAction::ToggleCheatsheet => {
            app.toggle_cheatsheet();
        }
        CommandAction::SwapWithSidebar => {
            app.layout.swap_focused_with(PanelId::Sidebar);
            resize_ptys(app);
        }
        CommandAction::SwapWithOpencode => {
            app.layout.swap_focused_with(PanelId::TerminalPane);
            resize_ptys(app);
        }
        CommandAction::SwapWithTerminal => {
            app.layout.swap_focused_with(PanelId::IntegratedTerminal);
            resize_ptys(app);
        }
        CommandAction::SwapWithNeovim => {
            app.layout.swap_focused_with(PanelId::NeovimPane);
            resize_ptys(app);
        }
        CommandAction::SwapWithGit => {
            app.layout.swap_focused_with(PanelId::GitPanel);
            resize_ptys(app);
        }
        CommandAction::SessionSelector => {
            app.open_session_selector();
        }
        CommandAction::ToggleTodoPanel => {
            if app.todo_panel.is_some() {
                app.close_todo_panel();
            } else if let Some(project) = app.projects.get(app.active_project) {
                if let Some(ref session_id) = project.active_session {
                    let proj_dir = project.path.to_string_lossy().to_string();
                    let sid = session_id.clone();
                    let base_url = crate::app::base_url().to_string();
                    let bg_tx = app.bg_tx.clone();
                    tokio::spawn(async move {
                        let client = crate::api::ApiClient::new();
                        if let Ok(todos) = client.fetch_todos(&base_url, &proj_dir, &sid).await {
                            let _ = bg_tx.send(BackgroundEvent::TodosFetched {
                                session_id: sid.clone(),
                                todos,
                            });
                        }
                    });
                    app.todo_panel = Some(crate::app::TodoPanelState {
                        todos: Vec::new(),
                        selected: 0,
                        scroll_offset: 0,
                        session_id: session_id.clone(),
                        editing: None,
                        dirty: false,
                    });
                }
            }
        }
        CommandAction::ContextInput => {
            if app.context_input.is_some() {
                app.context_input = None;
            } else {
                app.context_input = Some(crate::app::ContextInputState::new());
            }
        }
        CommandAction::NewTerminalTab => {
            app.add_shell_tab();
            app.layout.set_visible(PanelId::IntegratedTerminal, true);
            app.layout.focused = PanelId::IntegratedTerminal;
            resize_ptys(app);
        }
        CommandAction::NextTerminalTab => {
            if let Some(project) = app.projects.get_mut(app.active_project) {
                if let Some(resources) = project.active_resources_mut() {
                    if !resources.shell_ptys.is_empty() {
                        resources.active_shell_tab =
                            (resources.active_shell_tab + 1) % resources.shell_ptys.len();
                    }
                }
            }
        }
        CommandAction::PrevTerminalTab => {
            if let Some(project) = app.projects.get_mut(app.active_project) {
                if let Some(resources) = project.active_resources_mut() {
                    if !resources.shell_ptys.is_empty() {
                        resources.active_shell_tab = if resources.active_shell_tab == 0 {
                            resources.shell_ptys.len() - 1
                        } else {
                            resources.active_shell_tab - 1
                        };
                    }
                }
            }
        }
        CommandAction::CloseTerminalTab => {
            let mut should_hide = false;
            if let Some(project) = app.projects.get_mut(app.active_project) {
                if let Some(resources) = project.active_resources_mut() {
                    if resources.shell_ptys.len() > 1 {
                        resources.shell_ptys.remove(resources.active_shell_tab);
                        if resources.active_shell_tab >= resources.shell_ptys.len() {
                            resources.active_shell_tab =
                                resources.shell_ptys.len().saturating_sub(1);
                        }
                    } else if resources.shell_ptys.len() == 1 {
                        resources.shell_ptys.clear();
                        resources.active_shell_tab = 0;
                        should_hide = true;
                    }
                }
            }
            if should_hide {
                app.layout.set_visible(PanelId::IntegratedTerminal, false);
                resize_ptys(app);
            }
        }
        CommandAction::SearchTerminal => {
            app.terminal_search = Some(crate::app::TerminalSearchState {
                query: String::new(),
                cursor: 0,
                matches: Vec::new(),
                current_match: 0,
            });
        }
        CommandAction::SearchNextMatch => {
            if let Some(ref mut search) = app.terminal_search {
                if !search.matches.is_empty() {
                    search.current_match = (search.current_match + 1) % search.matches.len();
                    let (match_row, _, _) = search.matches[search.current_match];
                    if let Some(project) = app.projects.get_mut(app.active_project) {
                        if let Some(pty) = project.active_shell_pty_mut() {
                            if let Ok(mut p) = pty.parser.lock() {
                                let rows = p.screen().size().0 as usize;
                                if match_row >= rows {
                                    let target_scrollback = match_row - rows + 1;
                                    p.set_scrollback(target_scrollback);
                                    pty.scroll_offset = p.screen().scrollback();
                                }
                            }
                        }
                    }
                }
            }
        }
        CommandAction::SearchPrevMatch => {
            if let Some(ref mut search) = app.terminal_search {
                if !search.matches.is_empty() {
                    search.current_match = if search.current_match == 0 {
                        search.matches.len() - 1
                    } else {
                        search.current_match - 1
                    };
                    let (match_row, _, _) = search.matches[search.current_match];
                    if let Some(project) = app.projects.get_mut(app.active_project) {
                        if let Some(pty) = project.active_shell_pty_mut() {
                            if let Ok(mut p) = pty.parser.lock() {
                                let rows = p.screen().size().0 as usize;
                                if match_row >= rows {
                                    let target_scrollback = match_row - rows + 1;
                                    p.set_scrollback(target_scrollback);
                                    pty.scroll_offset = p.screen().scrollback();
                                }
                            }
                        }
                    }
                }
            }
        }
        // Actions already handled in command_action.rs — should not reach here
        _ => {}
    }
    Ok(())
}
