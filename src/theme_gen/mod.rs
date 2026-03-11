use std::path::PathBuf;

use ratatui::style::Color;

use crate::theme::ThemeColors;

mod nvim;
mod nvim_ext;
mod shell_themes;

pub use nvim::generate_nvim_colorscheme;
pub use shell_themes::{generate_gitui_theme, generate_zsh_theme};

pub(crate) fn color_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

pub fn theme_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/opman/themes")
}

#[allow(dead_code)]
pub fn zdotdir_path() -> PathBuf {
    theme_dir().join("zdotdir")
}

pub fn write_theme_files(theme: &ThemeColors) -> std::io::Result<PathBuf> {
    let dir = theme_dir();
    std::fs::create_dir_all(&dir)?;

    let nvim_dir = dir.join("nvim/colors");
    std::fs::create_dir_all(&nvim_dir)?;
    std::fs::write(
        nvim_dir.join("opencode.lua"),
        generate_nvim_colorscheme(theme),
    )?;

    let nvim_init = dir.join("nvim/init.lua");
    std::fs::write(&nvim_init, "vim.cmd('colorscheme opencode')\n")?;

    std::fs::write(dir.join("opencode.zsh"), generate_zsh_theme(theme))?;

    let gitui_dir = dir.join("gitui");
    std::fs::create_dir_all(&gitui_dir)?;
    std::fs::write(gitui_dir.join("opencode.ron"), generate_gitui_theme(theme))?;

    write_zdotdir(&dir)?;
    write_bash_integration(&dir)?;

    Ok(dir)
}

fn write_zdotdir(theme_dir: &std::path::Path) -> std::io::Result<()> {
    let zdotdir = theme_dir.join("zdotdir");
    std::fs::create_dir_all(&zdotdir)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let theme_file = theme_dir.join("opencode.zsh");

    let zshrc = format!(
        r#"# OpenCode Manager shell wrapper (auto-generated, do not edit)
# Restore original ZDOTDIR so user's config paths resolve correctly
if [[ -n "$OPENCODE_ORIG_ZDOTDIR" ]]; then
  export ZDOTDIR="$OPENCODE_ORIG_ZDOTDIR"
  unset OPENCODE_ORIG_ZDOTDIR
else
  unset ZDOTDIR
fi

# Source user's original zshenv (if not already sourced by zsh startup)
[[ -f "{home}/.zshenv" ]] && source "{home}/.zshenv"

# Source global zshrc
[[ -f "/etc/zshrc" ]] && source "/etc/zshrc"

# Source user's original zshrc
if [[ -n "$ZDOTDIR" ]] && [[ -f "$ZDOTDIR/.zshrc" ]]; then
  source "$ZDOTDIR/.zshrc"
elif [[ -f "{home}/.zshrc" ]]; then
  source "{home}/.zshrc"
fi

# Apply OpenCode theme (LAST, so our colors override)
[[ -f "{theme_file}" ]] && source "{theme_file}"

# Shell integration: emit OSC 133 sequences for command state tracking
__opencode_preexec() {{ printf '\x1b]133;B\x07' }}
__opencode_precmd() {{
  local ec=$?
  printf '\x1b]133;D;%d\x07' "$ec"
  printf '\x1b]133;A\x07'
}}
autoload -Uz add-zsh-hook
add-zsh-hook precmd __opencode_precmd
add-zsh-hook preexec __opencode_preexec
"#,
        home = home,
        theme_file = theme_file.display(),
    );

    std::fs::write(zdotdir.join(".zshrc"), zshrc)?;

    // Also create a .zshenv that just restores ZDOTDIR early for any scripts
    // that check it during env setup
    let zshenv = format!(
        r#"# OpenCode Manager zshenv wrapper (auto-generated)
if [[ -n "$OPENCODE_ORIG_ZDOTDIR" ]]; then
  export ZDOTDIR="$OPENCODE_ORIG_ZDOTDIR"
fi
[[ -f "{home}/.zshenv" ]] && source "{home}/.zshenv"
"#,
        home = home,
    );
    std::fs::write(zdotdir.join(".zshenv"), zshenv)?;

    Ok(())
}

/// Write a bash integration script that emits OSC 133 sequences for command state tracking.
pub fn write_bash_integration(theme_dir: &std::path::Path) -> std::io::Result<()> {
    let script = r#"# OpenCode Manager bash integration (auto-generated, do not edit)
# Source user's bashrc first
[ -f ~/.bashrc ] && source ~/.bashrc

# OSC 133 shell integration for command state tracking
__opencode_prompt_command() {
    local ec=$?
    printf '\033]133;D;%d\007' "$ec"
    printf '\033]133;A\007'
}
PROMPT_COMMAND="__opencode_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
trap 'printf "\033]133;B\007"' DEBUG
"#;
    std::fs::write(theme_dir.join("bash_integration.sh"), script)?;
    Ok(())
}
