use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, InputMode};
use crate::command_palette::CommandAction;
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;

use super::resize_ptys;

pub(super) fn handle_which_key_input(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.which_key.deactivate();
            app.vim_mode = VimMode::Normal;
        }
        _ => {
            if let Some(action) = app.which_key.process_key(&key) {
                app.vim_mode = VimMode::Normal;
                execute_command_action(app, action)?;
            } else if !app.which_key.active {
                app.vim_mode = VimMode::Normal;
            }
        }
    }
    Ok(())
}

pub(super) fn handle_command_palette_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.vim_mode = VimMode::Normal;
            app.command_palette.reset();
        }
        KeyCode::Enter => {
            if let Some(action) = app.command_palette.selected_action() {
                app.vim_mode = VimMode::Normal;
                app.command_palette.reset();
                execute_command_action(app, action)?;
            }
        }
        KeyCode::Up => app.command_palette.move_up(),
        KeyCode::Down => app.command_palette.move_down(),
        KeyCode::Backspace => {
            app.command_palette.backspace();
            app.command_palette.tick();
        }
        KeyCode::Left => app.command_palette.cursor_left(),
        KeyCode::Right => app.command_palette.cursor_right(),
        KeyCode::Char(c) => {
            app.command_palette.insert_char(c);
            app.command_palette.tick();
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn execute_command_action(app: &mut App, action: CommandAction) -> Result<()> {
    match action {
        CommandAction::ToggleSidebar => {
            app.layout.toggle_visible(PanelId::Sidebar);
            resize_ptys(app);
        }
        CommandAction::ToggleTerminal => {
            app.layout.toggle_visible(PanelId::IntegratedTerminal);
            if app.layout.is_visible(PanelId::IntegratedTerminal) {
                app.layout.focused = PanelId::IntegratedTerminal;
                app.ensure_shell_pty();
            }
            resize_ptys(app);
        }
        CommandAction::NavigateLeft => app.layout.navigate_left(),
        CommandAction::NavigateRight => app.layout.navigate_right(),
        CommandAction::NavigateUp => app.layout.navigate_up(),
        CommandAction::NavigateDown => app.layout.navigate_down(),
        CommandAction::SwapTerminal => {
            if let Some(project) = app.projects.get_mut(app.active_project) {
                if let Some(sid) = project.active_session.clone() {
                    let active_tab = project
                        .session_resources
                        .get(&sid)
                        .map(|r| r.active_shell_tab)
                        .unwrap_or(0);
                    if let (Some(pty), Some(resources)) = (
                        project.ptys.get_mut(&sid),
                        project.session_resources.get_mut(&sid),
                    ) {
                        if let Some(shell) = resources.shell_ptys.get_mut(active_tab) {
                            std::mem::swap(pty, shell);
                        }
                    }
                }
            }
        }
        CommandAction::ToggleGitPanel => {
            app.layout.toggle_visible(PanelId::GitPanel);
            if app.layout.is_visible(PanelId::GitPanel) {
                app.layout.focused = PanelId::GitPanel;
                app.ensure_gitui_pty();
            }
            resize_ptys(app);
        }
        CommandAction::FuzzyPicker => {
            app.start_add_project();
        }
        CommandAction::AddProject => {
            app.input_mode = InputMode::AddProject;
            app.input_buffer.clear();
        }
        CommandAction::SearchSessions => {
            app.open_session_search();
        }
        CommandAction::ToggleNeovim => {
            app.layout.toggle_visible(PanelId::NeovimPane);
            if app.layout.is_visible(PanelId::NeovimPane) {
                app.layout.focused = PanelId::NeovimPane;
                app.ensure_neovim_pty();
            }
            resize_ptys(app);
        }
        CommandAction::ZenMode => {
            app.zen_mode = !app.zen_mode;
            if app.zen_mode {
                let focused = app.layout.focused;
                app.pre_zen_state = Some((app.layout.panel_visible, focused));
                for panel in &[
                    PanelId::Sidebar,
                    PanelId::TerminalPane,
                    PanelId::NeovimPane,
                    PanelId::IntegratedTerminal,
                    PanelId::GitPanel,
                ] {
                    if *panel != focused {
                        app.layout.set_visible(*panel, false);
                    }
                }
            } else if let Some((saved_visible, saved_focused)) = app.pre_zen_state.take() {
                for (i, panel) in [
                    PanelId::Sidebar,
                    PanelId::TerminalPane,
                    PanelId::NeovimPane,
                    PanelId::IntegratedTerminal,
                    PanelId::GitPanel,
                ]
                .iter()
                .enumerate()
                {
                    app.layout.set_visible(*panel, saved_visible[i]);
                }
                app.layout.focused = saved_focused;
            } else {
                app.layout.set_visible(PanelId::Sidebar, true);
                app.layout.set_visible(PanelId::TerminalPane, true);
            }
            resize_ptys(app);
        }
        CommandAction::ZenTerminal => {
            super::popout::zen_panel(app, PanelId::IntegratedTerminal);
        }
        CommandAction::ZenOpencode => {
            super::popout::zen_panel(app, PanelId::TerminalPane);
        }
        CommandAction::ZenNeovim => {
            super::popout::zen_panel(app, PanelId::NeovimPane);
            if app.zen_mode {
                app.ensure_neovim_pty();
            }
        }
        CommandAction::ZenGit => {
            super::popout::zen_panel(app, PanelId::GitPanel);
        }
        CommandAction::PopOutPanels => {
            super::popout::popout_panels(app);
        }
        CommandAction::SessionWatcher => {
            if app.watcher_modal.is_some() {
                app.watcher_modal = None;
            } else {
                super::watcher::open_watcher_modal(app);
            }
        }
        // Delegate remaining arms to command_action_ext
        _ => {
            return super::command_action_ext::execute_command_action_ext(app, action);
        }
    }
    Ok(())
}
