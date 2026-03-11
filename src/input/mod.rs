mod command_action;
mod command_action_ext;
mod command_action_slack;
mod config;
mod context;
mod mouse;
mod overlays;
mod popout;
mod pty_keys;
mod sidebar;
mod todo;
mod watcher;
mod watcher_keys;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, InputMode};
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;
use crate::which_key::{lookup_binding, BindingMatch};

pub use mouse::mouse_event_to_bytes;

pub fn resize_ptys(app: &mut App) {
    app.resize_all_ptys();
}

/// Process a key event and update app state accordingly.
///
/// Routing rules:
/// - `Ctrl+b`: Toggle sidebar visibility (global).
/// - `Ctrl+c` / `Ctrl+q`: Quit the application (global).
/// - When `Focus::Sidebar`: Handle navigation keys locally.
/// - When `Focus::TerminalPane`: Forward all keys to the active PTY.
/// - When `Focus::IntegratedTerminal`: Forward all keys to the shell PTY.
pub fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<()> {
    // Command mode: route to command palette
    if app.vim_mode == VimMode::Command {
        return command_action::handle_command_palette_keys(app, key);
    }

    // WhichKey mode: route to which-key processor
    if app.vim_mode == VimMode::WhichKey {
        return command_action::handle_which_key_input(app, key);
    }

    // Insert mode: handle Escape tracking for mode switch
    if app.vim_mode == VimMode::Insert && key.code == KeyCode::Esc {
        app.escape_tracker.record_press();
        let is_terminal = matches!(
            app.layout.focused,
            PanelId::TerminalPane | PanelId::NeovimPane | PanelId::IntegratedTerminal
        );
        if is_terminal {
            if app.escape_tracker.triggered_double() {
                app.vim_mode = VimMode::Normal;
                return Ok(());
            }
        } else {
            if app.escape_tracker.triggered_double() {
                app.vim_mode = VimMode::Normal;
                return Ok(());
            }
        }
    }

    // Overlay handlers — these consume all keys when active
    if app.input_mode == InputMode::FuzzyPicker {
        return overlays::handle_fuzzy_picker_keys(app, key);
    }

    if app.input_mode == InputMode::AddProject {
        return overlays::handle_add_project_keys(app, key);
    }

    if app.session_search_mode {
        return overlays::handle_session_search_keys(app, key);
    }

    if app.show_config_panel {
        return config::handle_config_panel_keys(app, key);
    }

    if app.show_slack_log {
        return config::handle_slack_log_keys(app, key);
    }

    if app.session_selector.is_some() {
        return overlays::handle_session_selector_keys(app, &key);
    }

    if app.watcher_modal.is_some() {
        return watcher::handle_watcher_modal_keys(app, key);
    }

    if app.todo_panel.is_some() {
        return todo::handle_todo_panel_keys(app, key);
    }

    if app.context_input.is_some() {
        return context::handle_context_input_keys(app, key);
    }

    if app.confirm_delete.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.pending_remove = app.confirm_delete.take();
            }
            _ => {
                app.confirm_delete = None;
            }
        }
        return Ok(());
    }

    if app.show_cheatsheet {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                app.show_cheatsheet = false;
                return Ok(());
            }
            _ => {
                app.show_cheatsheet = false;
            }
        }
    }

    if app.vim_mode == VimMode::Resize && key.code == KeyCode::Esc {
        app.vim_mode = VimMode::Normal;
        return Ok(());
    }

    // Registry-based keybind lookup
    let has_modifier = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    let on_sidebar_or_git = matches!(app.layout.focused, PanelId::Sidebar | PanelId::GitPanel);
    let is_leader_key = key.code == KeyCode::Char(' ') && app.vim_mode == VimMode::Normal;

    // Skip registry for non-modifier keys when Sidebar/GitPanel focused (let panel handlers work)
    // BUT always allow leader key (Space) through so which-key works everywhere
    // Also skip for Insert mode non-modifier keys (pass through to PTY)
    let should_lookup = if has_modifier {
        true
    } else if is_leader_key {
        true
    } else if on_sidebar_or_git {
        false
    } else if app.vim_mode == VimMode::Insert {
        false
    } else {
        app.input_mode == InputMode::Normal
            && !app.session_search_mode
            && app.confirm_delete.is_none()
    };

    if should_lookup {
        match lookup_binding(&key, app.vim_mode, &app.runtime_keymap) {
            BindingMatch::Leaf(action) => {
                return command_action::execute_command_action(app, action);
            }
            BindingMatch::Prefix(label, children) => {
                app.vim_mode = VimMode::WhichKey;
                app.which_key.activate_with(label, children);
                return Ok(());
            }
            BindingMatch::None => {}
        }
    }

    // For Insert mode, check always-active bindings (Ctrl+q, Ctrl+/) even when skipped above
    if app.vim_mode == VimMode::Insert && has_modifier {
        match lookup_binding(&key, app.vim_mode, &app.runtime_keymap) {
            BindingMatch::Leaf(action) => {
                return command_action::execute_command_action(app, action);
            }
            _ => {}
        }
    }

    // Route based on current focus
    match app.layout.focused {
        PanelId::Sidebar => sidebar::handle_sidebar_keys(app, key),
        PanelId::TerminalPane => pty_keys::handle_terminal_keys(app, key),
        PanelId::NeovimPane => pty_keys::handle_neovim_keys(app, key),
        PanelId::IntegratedTerminal => pty_keys::handle_integrated_terminal_keys(app, key),
        PanelId::GitPanel => pty_keys::handle_git_panel_keys(app, key),
    }
}

pub fn handle_paste(app: &mut App, text: &str) {
    // Route paste to overlay panels first — these consume all input when active

    // Todo panel: only accepts paste when actively editing a todo
    if let Some(ref mut state) = app.todo_panel {
        if let Some(ref mut editing) = state.editing {
            editing.buffer.insert_str(editing.cursor_pos, text);
            editing.cursor_pos += text.len();
            return;
        }
        // Todo panel is open but not editing — swallow paste (don't send to PTY)
        return;
    }

    // Context input overlay: insert pasted text (may contain newlines)
    if let Some(ref mut state) = app.context_input {
        for c in text.chars() {
            if c == '\n' || c == '\r' {
                state.insert_newline();
            } else {
                state.insert_char(c);
            }
        }
        return;
    }

    // Session selector: insert pasted text into search query
    if let Some(ref mut state) = app.session_selector {
        for c in text.chars() {
            if c != '\n' && c != '\r' {
                state.insert_char(c);
            }
        }
        return;
    }

    // Session search mode: insert pasted text into search buffer
    if app.session_search_mode {
        for c in text.chars() {
            if c != '\n' && c != '\r' {
                app.session_search_buffer
                    .insert(app.session_search_cursor, c);
                app.session_search_cursor += 1;
            }
        }
        app.update_session_search();
        return;
    }

    // Add project input: insert pasted text into input buffer
    if app.input_mode == InputMode::AddProject {
        for c in text.chars() {
            if c != '\n' && c != '\r' {
                app.input_buffer.insert(app.input_cursor, c);
                app.input_cursor += 1;
            }
        }
        app.update_completions();
        return;
    }

    // Fuzzy picker: insert pasted text into search query
    if app.input_mode == InputMode::FuzzyPicker {
        if let Some(ref mut state) = app.fuzzy_picker {
            for c in text.chars() {
                if c != '\n' && c != '\r' {
                    state.insert_char(c);
                }
            }
            state.tick();
        }
        return;
    }

    // No overlay active — forward paste to the focused PTY panel
    let focused = app.layout.focused;
    let Some(project) = app.active_project_mut() else {
        return;
    };
    let bracketed = format!("\x1b[200~{}\x1b[201~", text);
    let bytes = bracketed.as_bytes();
    match focused {
        PanelId::TerminalPane => {
            if let Some(pty) = project.active_pty_mut() {
                let _ = pty.write(bytes);
            }
        }
        PanelId::NeovimPane => {
            if let Some(pty) = project
                .active_resources_mut()
                .and_then(|r| r.neovim_pty.as_mut())
            {
                let _ = pty.write(bytes);
            }
        }
        PanelId::IntegratedTerminal => {
            if let Some(pty) = project.active_shell_pty_mut() {
                let _ = pty.write(bytes);
            }
        }
        PanelId::GitPanel => {
            if let Some(pty) = project.gitui_pty.as_mut() {
                let _ = pty.write(bytes);
            }
        }
        _ => {}
    }
}
