use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, BackgroundEvent, InputMode, SidebarItem};
use crate::command_palette::CommandAction;
use crate::ui::layout_manager::PanelId;
use crate::vim_mode::VimMode;
use crate::which_key::{lookup_binding, BindingMatch};

pub fn resize_ptys(app: &mut App) {
    app.resize_all_ptys();
}

fn zen_panel(app: &mut App, target: PanelId) {
    if app.zen_mode {
        if let Some((saved_visible, saved_focused)) = app.pre_zen_state.take() {
            app.layout.panel_visible = saved_visible;
            app.layout.focused = saved_focused;
        } else {
            app.layout.set_visible(PanelId::Sidebar, true);
            app.layout.set_visible(PanelId::TerminalPane, true);
        }
        app.zen_mode = false;
    } else {
        let focused = app.layout.focused;
        app.pre_zen_state = Some((app.layout.panel_visible, focused));
        for panel in &[
            PanelId::Sidebar,
            PanelId::TerminalPane,
            PanelId::NeovimPane,
            PanelId::IntegratedTerminal,
            PanelId::GitPanel,
        ] {
            app.layout.set_visible(*panel, *panel == target);
        }
        app.layout.focused = target;
        app.zen_mode = true;
    }
    resize_ptys(app);
}

fn popout_panels(app: &mut App) {
    if app.popout_mode {
        for child in app.popout_windows.drain(..) {
            kill_process_tree(child);
        }
        if let Some((saved_visible, saved_focused)) = app.pre_popout_state.take() {
            app.layout.panel_visible = saved_visible;
            app.layout.focused = saved_focused;
        } else {
            app.layout.set_visible(PanelId::Sidebar, true);
            app.layout.set_visible(PanelId::TerminalPane, true);
        }
        app.popout_mode = false;
        app.toast_message = Some(("Panels restored".into(), std::time::Instant::now()));
    } else {
        let project = match app.projects.get(app.active_project) {
            Some(p) => p,
            None => return,
        };
        let project_dir = project.path.clone();
        let theme_envs = app.theme.pty_env_vars();
        let td = crate::theme_gen::theme_dir();

        let focused = app.layout.focused;
        app.pre_popout_state = Some((app.layout.panel_visible, focused));

        let panels_to_popout: Vec<PanelId> = [
            PanelId::TerminalPane,
            PanelId::NeovimPane,
            PanelId::IntegratedTerminal,
            PanelId::GitPanel,
        ]
        .iter()
        .copied()
        .filter(|p| app.layout.is_visible(*p))
        .collect();

        if panels_to_popout.is_empty() {
            app.pre_popout_state = None;
            app.toast_message = Some((
                "No panels visible to pop out".into(),
                std::time::Instant::now(),
            ));
            return;
        }

        let mut spawned: Vec<std::process::Child> = Vec::new();

        for panel in &panels_to_popout {
            let cmd_str = match panel {
                PanelId::TerminalPane => {
                    let base_url = crate::app::base_url();
                    let dir = project_dir.to_string_lossy();
                    let session_part = project
                        .active_session
                        .as_ref()
                        .map(|sid| format!(" --session {}", sid))
                        .unwrap_or_default();
                    format!("opencode attach {} --dir {}{}", base_url, dir, session_part)
                }
                PanelId::NeovimPane => {
                    let colorscheme_path = td.join("nvim/colors/opencode.lua");
                    if colorscheme_path.exists() {
                        format!(
                            "nvim --cmd 'autocmd VimEnter * ++once silent! luafile {}'",
                            colorscheme_path.display()
                        )
                    } else {
                        "nvim".into()
                    }
                }
                PanelId::IntegratedTerminal => {
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
                }
                PanelId::GitPanel => {
                    let gitui_theme = td.join("gitui/opencode.ron");
                    if gitui_theme.exists() {
                        format!("gitui -t {}", gitui_theme.display())
                    } else {
                        "gitui".into()
                    }
                }
                _ => continue,
            };

            let title = match panel {
                PanelId::TerminalPane => "OpenCode",
                PanelId::NeovimPane => "Neovim",
                PanelId::IntegratedTerminal => "Terminal",
                PanelId::GitPanel => "GitUI",
                _ => "Panel",
            };

            if let Some(child) = spawn_external_terminal(&project_dir, &cmd_str, title, &theme_envs)
            {
                spawned.push(child);
            }
        }

        if spawned.is_empty() {
            app.pre_popout_state = None;
            app.toast_message = Some((
                "Failed to spawn external windows".into(),
                std::time::Instant::now(),
            ));
            return;
        }

        for panel in &[
            PanelId::Sidebar,
            PanelId::TerminalPane,
            PanelId::NeovimPane,
            PanelId::IntegratedTerminal,
            PanelId::GitPanel,
        ] {
            app.layout.set_visible(*panel, false);
        }
        app.popout_windows = spawned;
        app.popout_mode = true;
        let count = panels_to_popout.len();
        app.toast_message = Some((
            format!(
                "{} panel{} popped out — Space+w+w to restore",
                count,
                if count == 1 { "" } else { "s" }
            ),
            std::time::Instant::now(),
        ));
    }
    resize_ptys(app);
}

fn spawn_external_terminal(
    cwd: &std::path::Path,
    command: &str,
    title: &str,
    theme_envs: &[(String, String)],
) -> Option<std::process::Child> {
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    let mut env_exports = String::new();
    env_exports.push_str("export TERM=xterm-256color COLORTERM=truecolor; ");
    for (key, val) in theme_envs {
        env_exports.push_str(&format!(
            "export {}='{}'; ",
            key,
            val.replace('\'', "'\\''")
        ));
    }

    let shell_cmd = format!("{}cd {} && {}", env_exports, shell_escape(cwd), command);

    if term_program.contains("iTerm") {
        spawn_iterm2(cwd, &shell_cmd, title)
    } else if term_program.contains("Alacritty") || which_exists("alacritty") {
        spawn_alacritty(cwd, &shell_cmd, title)
    } else if term_program.contains("WezTerm") || which_exists("wezterm") {
        spawn_wezterm(cwd, &shell_cmd, title)
    } else {
        spawn_macos_terminal(cwd, &shell_cmd, title)
    }
}

fn shell_escape(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if s.contains(' ') || s.contains('\'') || s.contains('"') || s.contains('\\') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn spawn_macos_terminal(
    _cwd: &std::path::Path,
    shell_cmd: &str,
    title: &str,
) -> Option<std::process::Child> {
    let script = format!(
        r#"tell application "Terminal"
    activate
    set newTab to do script "{}"
    set custom title of newTab to "{}"
end tell"#,
        shell_cmd.replace('\\', "\\\\").replace('"', "\\\""),
        title,
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn()
        .ok()
}

fn spawn_iterm2(
    _cwd: &std::path::Path,
    shell_cmd: &str,
    title: &str,
) -> Option<std::process::Child> {
    let script = format!(
        r#"tell application "iTerm2"
    activate
    set newWindow to (create window with default profile)
    tell current session of newWindow
        set name to "{}"
        write text "{}"
    end tell
end tell"#,
        title,
        shell_cmd.replace('\\', "\\\\").replace('"', "\\\""),
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn()
        .ok()
}

fn spawn_alacritty(
    cwd: &std::path::Path,
    shell_cmd: &str,
    title: &str,
) -> Option<std::process::Child> {
    std::process::Command::new("alacritty")
        .arg("--title")
        .arg(title)
        .arg("--working-directory")
        .arg(cwd)
        .arg("-e")
        .arg("sh")
        .arg("-c")
        .arg(shell_cmd)
        .spawn()
        .ok()
}

fn spawn_wezterm(
    cwd: &std::path::Path,
    shell_cmd: &str,
    _title: &str,
) -> Option<std::process::Child> {
    std::process::Command::new("wezterm")
        .arg("start")
        .arg("--cwd")
        .arg(cwd)
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg(shell_cmd)
        .spawn()
        .ok()
}

fn kill_process_tree(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
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
        return handle_command_palette_keys(app, key);
    }

    // WhichKey mode: route to which-key processor
    if app.vim_mode == VimMode::WhichKey {
        return handle_which_key_input(app, key);
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
        return handle_fuzzy_picker_keys(app, key);
    }

    if app.input_mode == InputMode::AddProject {
        return handle_add_project_keys(app, key);
    }

    if app.session_search_mode {
        return handle_session_search_keys(app, key);
    }

    if app.show_config_panel {
        return handle_config_panel_keys(app, key);
    }

    if app.session_selector.is_some() {
        return handle_session_selector_keys(app, &key);
    }

    if app.todo_panel.is_some() {
        return handle_todo_panel_keys(app, key);
    }

    if app.context_input.is_some() {
        return handle_context_input_keys(app, key);
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
                return execute_command_action(app, action);
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
                return execute_command_action(app, action);
            }
            _ => {}
        }
    }

    // Route based on current focus
    match app.layout.focused {
        PanelId::Sidebar => handle_sidebar_keys(app, key),
        PanelId::TerminalPane => handle_terminal_keys(app, key),
        PanelId::NeovimPane => handle_neovim_keys(app, key),
        PanelId::IntegratedTerminal => handle_integrated_terminal_keys(app, key),
        PanelId::GitPanel => handle_git_panel_keys(app, key),
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

/// Handle keys when the sidebar is focused.
fn handle_sidebar_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('g') => {
            if app.sidebar_pending_g {
                app.sidebar_cursor = 0;
                app.sidebar_pending_g = false;
            } else {
                app.sidebar_pending_g = true;
            }
        }
        KeyCode::Char('G') => {
            let max = app.sidebar_item_count().saturating_sub(1);
            app.sidebar_cursor = max;
            app.sidebar_pending_g = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.sidebar_cursor > 0 {
                app.sidebar_cursor -= 1;
            }
            app.sidebar_pending_g = false;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = app.sidebar_item_count().saturating_sub(1);
            if app.sidebar_cursor < max {
                app.sidebar_cursor += 1;
            }
            app.sidebar_pending_g = false;
        }
        KeyCode::Enter => {
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
        KeyCode::Char('a') => {
            app.sidebar_pending_g = false;
            app.start_add_project();
        }
        KeyCode::Char('o') => {
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
        KeyCode::Char('r') => {
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
        KeyCode::Char('d') => {
            app.sidebar_pending_g = false;
            if let Some(SidebarItem::Project(idx)) = app.sidebar_item_at(app.sidebar_cursor) {
                app.confirm_delete = Some(idx);
            }
        }
        KeyCode::Char('?') => {
            app.sidebar_pending_g = false;
            app.toggle_cheatsheet();
        }
        KeyCode::Char('q') => {
            app.sidebar_pending_g = false;
            app.should_quit = true;
        }
        KeyCode::Esc => {
            app.sidebar_pending_g = false;
            app.vim_mode = VimMode::Normal;
        }
        _ => {
            app.sidebar_pending_g = false;
        }
    }
    Ok(())
}

/// Handle keys when the terminal pane is focused.
///
/// Most keys are forwarded directly to the PTY child process.
fn handle_terminal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    // Forward the key to the active project's PTY
    if let Some(project) = app.active_project_mut() {
        if let Some(pty) = project.active_pty_mut() {
            if pty.scroll_offset > 0 {
                pty.scroll_offset = 0;
                if let Ok(mut parser) = pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                pty.write(&bytes)?;
            }
        }
    }

    Ok(())
}

/// Convert a crossterm `KeyEvent` to bytes suitable for writing to a PTY.
///
/// Full xterm-compatible implementation supporting all modifiers and key codes.
fn key_event_to_bytes(key: &KeyEvent) -> Vec<u8> {
    let has_alt = key.modifiers.contains(KeyModifiers::ALT);
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

    // Compute xterm modifier parameter: 1 + (shift?1:0) + (alt?2:0) + (ctrl?4:0)
    let modifier_param = 1
        + if has_shift { 1 } else { 0 }
        + if has_alt { 2 } else { 0 }
        + if has_ctrl { 4 } else { 0 };
    let has_modifiers = modifier_param > 1;

    match key.code {
        KeyCode::Char(c) => {
            if has_ctrl && !has_alt {
                // Ctrl+letter → control character (0x01-0x1A)
                let ctrl_byte = (c.to_ascii_lowercase() as u8)
                    .wrapping_sub(b'a')
                    .wrapping_add(1);
                vec![ctrl_byte]
            } else if has_ctrl && has_alt {
                // Ctrl+Alt+letter → ESC + control character
                let ctrl_byte = (c.to_ascii_lowercase() as u8)
                    .wrapping_sub(b'a')
                    .wrapping_add(1);
                vec![0x1b, ctrl_byte]
            } else if has_alt {
                // Alt+key → ESC prefix + character
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(s.as_bytes());
                bytes
            } else {
                // Plain character (including Shift which is already reflected in `c`)
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => {
            if has_alt {
                vec![0x1b, b'\r']
            } else {
                vec![b'\r']
            }
        }
        KeyCode::Backspace => {
            if has_alt {
                vec![0x1b, 0x7f]
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => vec![0x1b, b'[', b'Z'], // Shift+Tab
        KeyCode::Esc => vec![0x1b],

        // Arrow keys: plain=\e[X, with modifiers=\e[1;{mod}X
        KeyCode::Up => {
            if has_modifiers {
                format!("\x1b[1;{}A", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'A']
            }
        }
        KeyCode::Down => {
            if has_modifiers {
                format!("\x1b[1;{}B", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'B']
            }
        }
        KeyCode::Right => {
            if has_modifiers {
                format!("\x1b[1;{}C", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'C']
            }
        }
        KeyCode::Left => {
            if has_modifiers {
                format!("\x1b[1;{}D", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'D']
            }
        }

        // Home/End: plain=\e[H/\e[F, with modifiers=\e[1;{mod}H/F
        KeyCode::Home => {
            if has_modifiers {
                format!("\x1b[1;{}H", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'H']
            }
        }
        KeyCode::End => {
            if has_modifiers {
                format!("\x1b[1;{}F", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'F']
            }
        }

        // Tilde-style keys: \e[{code}~ or \e[{code};{mod}~
        KeyCode::Insert => {
            if has_modifiers {
                format!("\x1b[2;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'2', b'~']
            }
        }
        KeyCode::Delete => {
            if has_modifiers {
                format!("\x1b[3;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'3', b'~']
            }
        }
        KeyCode::PageUp => {
            if has_modifiers {
                format!("\x1b[5;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'5', b'~']
            }
        }
        KeyCode::PageDown => {
            if has_modifiers {
                format!("\x1b[6;{}~", modifier_param).into_bytes()
            } else {
                vec![0x1b, b'[', b'6', b'~']
            }
        }

        // Function keys: F1-F4 use SS3 (plain) or CSI with modifiers
        // F5-F12 use tilde-style sequences
        KeyCode::F(n) => {
            let (code, letter) = match n {
                1 => (None, Some(b'P')), // \eOP or \e[1;{mod}P
                2 => (None, Some(b'Q')), // \eOQ
                3 => (None, Some(b'R')), // \eOR
                4 => (None, Some(b'S')), // \eOS
                5 => (Some(15), None),   // \e[15~
                6 => (Some(17), None),   // \e[17~
                7 => (Some(18), None),   // \e[18~
                8 => (Some(19), None),   // \e[19~
                9 => (Some(20), None),   // \e[20~
                10 => (Some(21), None),  // \e[21~
                11 => (Some(23), None),  // \e[23~
                12 => (Some(24), None),  // \e[24~
                _ => return Vec::new(),
            };
            match (code, letter) {
                (None, Some(l)) => {
                    if has_modifiers {
                        format!("\x1b[1;{}{}", modifier_param, l as char).into_bytes()
                    } else {
                        vec![0x1b, b'O', l]
                    }
                }
                (Some(c), None) => {
                    if has_modifiers {
                        format!("\x1b[{};{}~", c, modifier_param).into_bytes()
                    } else {
                        format!("\x1b[{}~", c).into_bytes()
                    }
                }
                _ => Vec::new(),
            }
        }

        _ => Vec::new(),
    }
}

fn handle_session_search_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.close_session_search();
        }
        KeyCode::Enter => {
            if let Some(session_id) = app.pin_selected_session() {
                app.pending_session_select = Some((app.active_project, session_id));
                app.layout.focused = PanelId::TerminalPane;
            }
        }
        KeyCode::Up => {
            if app.session_search_selected > 0 {
                app.session_search_selected -= 1;
            }
        }
        KeyCode::Down => {
            let max = app.session_search_results.len().saturating_sub(1);
            if app.session_search_selected < max {
                app.session_search_selected += 1;
            }
        }
        KeyCode::Backspace => {
            if app.session_search_cursor > 0 {
                app.session_search_cursor -= 1;
                app.session_search_buffer.remove(app.session_search_cursor);
                app.update_session_search();
            }
        }
        KeyCode::Char(c) => {
            app.session_search_buffer
                .insert(app.session_search_cursor, c);
            app.session_search_cursor += 1;
            app.update_session_search();
        }
        _ => {}
    }
    Ok(())
}

fn handle_config_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.show_config_panel = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.config_panel_selected > 0 {
                app.config_panel_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = config_panel_setting_count() - 1;
            if app.config_panel_selected < max {
                app.config_panel_selected += 1;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            toggle_config_setting(app);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            adjust_config_setting(app, -5);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            adjust_config_setting(app, 5);
        }
        _ => {}
    }
    Ok(())
}

fn config_panel_setting_count() -> usize {
    2
}

fn toggle_config_setting(app: &mut App) {
    match app.config_panel_selected {
        0 => {
            app.config.settings.follow_edits_in_neovim =
                !app.config.settings.follow_edits_in_neovim;
        }
        _ => {}
    }
    if let Err(e) = app.config.save() {
        eprintln!("Failed to save config: {}", e);
    }
}

/// Adjust a numeric setting by `delta` (clamped to valid range).
/// Only applies to percentage-type settings; ignored for booleans.
fn adjust_config_setting(app: &mut App, delta: i16) {
    match app.config_panel_selected {
        1 => {
            let cur = app.config.settings.unfocused_dim_percent as i16;
            app.config.settings.unfocused_dim_percent =
                cur.saturating_add(delta).clamp(0, 100) as u8;
        }
        _ => return,
    }
    if let Err(e) = app.config.save() {
        eprintln!("Failed to save config: {}", e);
    }
}

fn handle_session_selector_keys(app: &mut App, key: &KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.session_selector = None;
            app.vim_mode = VimMode::Normal;
        }
        KeyCode::Enter => {
            let switch_info = app.session_selector.as_ref().and_then(|state| {
                if state.filtered.is_empty() {
                    return None;
                }
                let entry = &state.entries[state.filtered[state.selected]];
                Some((entry.project_idx, entry.session.id.clone()))
            });
            if let Some((project_idx, session_id)) = switch_info {
                app.active_project = project_idx;
                app.projects[project_idx].active_session = Some(session_id.clone());
                app.pending_session_select = Some((project_idx, session_id));
                app.session_selector = None;
                app.vim_mode = VimMode::Insert;
                app.layout.focused = PanelId::TerminalPane;
            }
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.session_selector {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.session_selector {
                state.move_down();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.session_selector {
                state.move_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.session_selector {
                state.move_down();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.session_selector {
                state.backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.session_selector {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.session_selector {
                state.cursor_right();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.session_selector {
                state.insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_git_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(ref mut gitui_pty) = project.gitui_pty {
            if gitui_pty.scroll_offset > 0 {
                gitui_pty.scroll_offset = 0;
                if let Ok(mut parser) = gitui_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                gitui_pty.write(&bytes)?;
            }
            return Ok(());
        }
    }
    Ok(())
}

fn handle_add_project_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            if app.completions_visible {
                app.completions_visible = false;
            } else {
                app.cancel_input();
            }
        }
        KeyCode::Enter => {
            if app.completions_visible {
                app.apply_completion();
            }
            app.confirm_add_project()?;
        }
        KeyCode::Tab => {
            if app.completions_visible && app.completions.len() == 1 {
                app.apply_completion();
                app.update_completions();
            } else if app.completions_visible && app.completions.len() > 1 {
                app.complete_common_prefix();
            } else {
                app.update_completions();
            }
        }
        KeyCode::Up => {
            if app.completions_visible && !app.completions.is_empty() {
                let len = app.completions.len();
                app.completion_selected = (app.completion_selected + len - 1) % len;
            }
        }
        KeyCode::Down => {
            if app.completions_visible && !app.completions.is_empty() {
                let len = app.completions.len();
                app.completion_selected = (app.completion_selected + 1) % len;
            }
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input_buffer.remove(app.input_cursor);
                app.update_completions();
            }
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input_buffer.len() {
                app.input_cursor += 1;
            }
        }
        KeyCode::Char(c) => {
            app.input_buffer.insert(app.input_cursor, c);
            app.input_cursor += 1;
            app.update_completions();
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys in the fuzzy picker overlay.
fn handle_fuzzy_picker_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.cancel_fuzzy_picker();
        }
        KeyCode::Enter => {
            if let Some(ref _state) = app.fuzzy_picker {
                if _state.selected_path().is_some() {
                    app.confirm_fuzzy_add_project()?;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_down();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.move_down();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.backspace();
                state.tick();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.cursor_right();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.fuzzy_picker {
                state.insert_char(c);
                state.tick();
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle keys when the neovim pane is focused.
///
/// Forwards all keys to the neovim PTY.
fn handle_neovim_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(ref mut nvim_pty) = project
            .active_resources_mut()
            .and_then(|r| r.neovim_pty.as_mut())
        {
            if nvim_pty.scroll_offset > 0 {
                nvim_pty.scroll_offset = 0;
                if let Ok(mut parser) = nvim_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                nvim_pty.write(&bytes)?;
            }
        }
    }
    Ok(())
}

/// Handle keys when the integrated terminal panel is focused.
///
/// Forwards all keys to the shell PTY, similar to terminal pane.
fn handle_integrated_terminal_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(project) = app.active_project_mut() {
        if let Some(shell_pty) = project.active_shell_pty_mut() {
            if shell_pty.scroll_offset > 0 {
                shell_pty.scroll_offset = 0;
                if let Ok(mut parser) = shell_pty.parser.lock() {
                    parser.set_scrollback(0);
                }
            }
            let bytes = key_event_to_bytes(&key);
            if !bytes.is_empty() {
                shell_pty.write(&bytes)?;
            }
        }
    }
    Ok(())
}

fn handle_which_key_input(app: &mut App, key: KeyEvent) -> Result<()> {
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

fn handle_command_palette_keys(app: &mut App, key: KeyEvent) -> Result<()> {
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

fn execute_command_action(app: &mut App, action: CommandAction) -> Result<()> {
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
            zen_panel(app, PanelId::IntegratedTerminal);
        }
        CommandAction::ZenOpencode => {
            zen_panel(app, PanelId::TerminalPane);
        }
        CommandAction::ZenNeovim => {
            zen_panel(app, PanelId::NeovimPane);
            if app.zen_mode {
                app.ensure_neovim_pty();
            }
        }
        CommandAction::ZenGit => {
            zen_panel(app, PanelId::GitPanel);
        }
        CommandAction::PopOutPanels => {
            popout_panels(app);
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
        CommandAction::SwapPanel => {
            app.layout.swap_focused_with_next();
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
    }
    Ok(())
}

fn handle_context_input_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.context_input = None;
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Submit: send text as a system message to the active OpenCode session
            if let Some(state) = app.context_input.take() {
                let text = state.to_string();
                if !text.trim().is_empty() {
                    if let Some(project) = app.projects.get(app.active_project) {
                        if let Some(ref session_id) = project.active_session {
                            let proj_dir = project.path.to_string_lossy().to_string();
                            let sid = session_id.clone();
                            let base_url = crate::app::base_url().to_string();
                            tracing::info!(
                                session_id = sid,
                                "Sending context input as system message"
                            );
                            tokio::spawn(async move {
                                let client = crate::api::ApiClient::new();
                                let msg = format!("[SYSTEM CONTEXT from user] {text}");
                                match client
                                    .send_system_message_async(&base_url, &proj_dir, &sid, &msg)
                                    .await
                                {
                                    Ok(()) => {
                                        tracing::info!("Context system message sent successfully")
                                    }
                                    Err(e) => tracing::error!(
                                        "Failed to send context system message: {e}"
                                    ),
                                }
                            });
                        }
                    }
                }
            }
        }
        KeyCode::Enter => {
            if let Some(ref mut state) = app.context_input {
                state.insert_newline();
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.context_input {
                state.backspace();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_right();
            }
        }
        KeyCode::Up => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_up();
            }
        }
        KeyCode::Down => {
            if let Some(ref mut state) = app.context_input {
                state.cursor_down();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.context_input {
                state.insert_char(c);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_todo_panel_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_editing = app
        .todo_panel
        .as_ref()
        .map(|s| s.editing.is_some())
        .unwrap_or(false);

    if is_editing {
        return handle_todo_edit_keys(app, key);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.close_todo_panel();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut state) = app.todo_panel {
                state.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut state) = app.todo_panel {
                state.move_down();
            }
        }
        KeyCode::Char(' ') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get_mut(state.selected) {
                    todo.status = match todo.status.as_str() {
                        "pending" => "in_progress",
                        "in_progress" => "completed",
                        _ => "pending",
                    }
                    .to_string();
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('n') => {
            if let Some(ref mut state) = app.todo_panel {
                state.editing = Some(crate::app::EditingState {
                    index: None,
                    buffer: String::new(),
                    cursor_pos: 0,
                    priority: "high".to_string(),
                });
                state.selected = state.todos.len();
            }
        }
        KeyCode::Char('e') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get(state.selected).cloned() {
                    state.editing = Some(crate::app::EditingState {
                        index: Some(state.selected),
                        buffer: todo.content.clone(),
                        cursor_pos: todo.content.len(),
                        priority: todo.priority.clone(),
                    });
                }
            }
        }
        KeyCode::Char('d') => {
            if let Some(ref mut state) = app.todo_panel {
                if !state.todos.is_empty() {
                    state.todos.remove(state.selected);
                    if state.selected >= state.todos.len() {
                        state.selected = state.todos.len().saturating_sub(1);
                    }
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('p') => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(todo) = state.todos.get_mut(state.selected) {
                    todo.priority = match todo.priority.as_str() {
                        "low" => "medium",
                        "medium" => "high",
                        _ => "low",
                    }
                    .to_string();
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('K') => {
            if let Some(ref mut state) = app.todo_panel {
                if state.selected > 0 {
                    state.todos.swap(state.selected, state.selected - 1);
                    state.selected -= 1;
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('J') => {
            if let Some(ref mut state) = app.todo_panel {
                if state.selected + 1 < state.todos.len() {
                    state.todos.swap(state.selected, state.selected + 1);
                    state.selected += 1;
                    state.dirty = true;
                    let session_id = state.session_id.clone();
                    let todos = state.todos.clone();
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                            tracing::error!("Failed to save todos: {e}");
                        }
                    });
                }
            }
        }
        KeyCode::Char('y') => {
            if let Some(ref state) = app.todo_panel {
                if let Some(todo) = state.todos.get(state.selected) {
                    let _ = std::process::Command::new("pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(ref mut stdin) = child.stdin {
                                stdin.write_all(todo.content.as_bytes())?;
                            }
                            child.wait()
                        });
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_todo_edit_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_editing = app
        .todo_panel
        .as_ref()
        .map(|s| s.editing.is_some())
        .unwrap_or(false);
    if !is_editing {
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            if let Some(ref mut state) = app.todo_panel {
                state.editing = None;
            }
        }
        KeyCode::Enter => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref editing) = state.editing {
                    let content = editing.buffer.trim().to_string();
                    let priority = editing.priority.clone();
                    let index = editing.index;
                    if !content.is_empty() {
                        if let Some(idx) = index {
                            if let Some(todo) = state.todos.get_mut(idx) {
                                todo.content = content;
                                todo.priority = priority;
                            }
                        } else {
                            state.todos.push(crate::app::TodoItem {
                                content,
                                status: "pending".to_string(),
                                priority,
                            });
                        }
                        state.dirty = true;
                        let session_id = state.session_id.clone();
                        let todos = state.todos.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Err(e) = crate::todo_db::save_todos_to_db(&session_id, &todos) {
                                tracing::error!("Failed to save todos: {e}");
                            }
                        });
                    }
                }
                state.editing = None;
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    editing.buffer.insert(editing.cursor_pos, c);
                    editing.cursor_pos += c.len_utf8();
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos > 0 {
                        let prev = editing.buffer[..editing.cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        editing.buffer.replace_range(prev..editing.cursor_pos, "");
                        editing.cursor_pos = prev;
                    }
                }
            }
        }
        KeyCode::Left => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos > 0 {
                        editing.cursor_pos = editing.buffer[..editing.cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
            }
        }
        KeyCode::Right => {
            if let Some(ref mut state) = app.todo_panel {
                if let Some(ref mut editing) = state.editing {
                    if editing.cursor_pos < editing.buffer.len() {
                        editing.cursor_pos = editing.buffer[editing.cursor_pos..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| editing.cursor_pos + i)
                            .unwrap_or(editing.buffer.len());
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Convert a crossterm `MouseEvent` to SGR (1006) encoded bytes for the PTY.
///
/// SGR encoding: `ESC [ < Cb ; Cx ; Cy M` (press) or `ESC [ < Cb ; Cx ; Cy m` (release)
pub fn mouse_event_to_bytes(event: &MouseEvent, panel_x: u16, panel_y: u16) -> Option<Vec<u8>> {
    let col = event.column.saturating_sub(panel_x) + 1;
    let row = event.row.saturating_sub(panel_y) + 1;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            Some(format!("\x1b[<0;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Down(MouseButton::Right) => {
            Some(format!("\x1b[<2;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            Some(format!("\x1b[<1;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Left) => {
            Some(format!("\x1b[<0;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Right) => {
            Some(format!("\x1b[<2;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            Some(format!("\x1b[<1;{};{}m", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            Some(format!("\x1b[<32;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            Some(format!("\x1b[<34;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            Some(format!("\x1b[<33;{};{}M", col, row).into_bytes())
        }
        MouseEventKind::ScrollUp => Some(format!("\x1b[<64;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollDown => Some(format!("\x1b[<65;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollLeft => Some(format!("\x1b[<66;{};{}M", col, row).into_bytes()),
        MouseEventKind::ScrollRight => Some(format!("\x1b[<67;{};{}M", col, row).into_bytes()),
        MouseEventKind::Moved => Some(format!("\x1b[<35;{};{}M", col, row).into_bytes()),
    }
}
