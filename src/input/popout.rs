use crate::app::App;
use crate::ui::layout_manager::PanelId;

use super::resize_ptys;

pub(super) fn zen_panel(app: &mut App, target: PanelId) {
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

pub(super) fn popout_panels(app: &mut App) {
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

pub(super) fn kill_process_tree(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}
