use std::fmt::Write;
use std::path::PathBuf;

use ratatui::style::Color;

use crate::theme::{color_to_hex, ThemeColors};

fn color_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

pub fn theme_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/opencode-manager/themes")
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

pub fn generate_nvim_colorscheme(colors: &ThemeColors) -> String {
    let primary = color_to_hex(colors.primary);
    let secondary = color_to_hex(colors.secondary);
    let accent = color_to_hex(colors.accent);
    let bg = color_to_hex(colors.background);
    let bg_panel = color_to_hex(colors.background_panel);
    let bg_elem = color_to_hex(colors.background_element);
    let text = color_to_hex(colors.text);
    let muted = color_to_hex(colors.text_muted);
    let border = color_to_hex(colors.border);
    let border_subtle = color_to_hex(colors.border_subtle);
    let error = color_to_hex(colors.error);
    let warning = color_to_hex(colors.warning);
    let success = color_to_hex(colors.success);
    let info = color_to_hex(colors.info);

    let mut s = String::with_capacity(16384);

    let _ = writeln!(s, "-- OpenCode Manager Theme (auto-generated, do not edit)");
    let _ = writeln!(s, "vim.cmd('highlight clear')");
    let _ = writeln!(s, "vim.o.termguicolors = true");
    let _ = writeln!(s, "vim.g.colors_name = 'opencode'");
    let _ = writeln!(s);
    let _ = writeln!(s, "local hi = function(group, opts)");
    let _ = writeln!(s, "  vim.api.nvim_set_hl(0, group, opts)");
    let _ = writeln!(s, "end");
    let _ = writeln!(s);

    // --- Editor UI ---
    let _ = writeln!(s, "-- Editor UI");
    let _ = writeln!(s, "hi('Normal', {{ fg = '{}', bg = '{}' }})", text, bg);
    let _ = writeln!(
        s,
        "hi('NormalFloat', {{ fg = '{}', bg = '{}' }})",
        text, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('FloatBorder', {{ fg = '{}', bg = '{}' }})",
        border, bg_panel
    );
    let _ = writeln!(s, "hi('CursorLine', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(
        s,
        "hi('CursorLineNr', {{ fg = '{}', bold = true }})",
        primary
    );
    let _ = writeln!(s, "hi('LineNr', {{ fg = '{}' }})", border_subtle);
    let _ = writeln!(s, "hi('Visual', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(s, "hi('Search', {{ fg = '{}', bg = '{}' }})", bg, warning);
    let _ = writeln!(
        s,
        "hi('IncSearch', {{ fg = '{}', bg = '{}' }})",
        bg, primary
    );
    let _ = writeln!(
        s,
        "hi('StatusLine', {{ fg = '{}', bg = '{}' }})",
        text, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('StatusLineNC', {{ fg = '{}', bg = '{}' }})",
        muted, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TabLine', {{ fg = '{}', bg = '{}' }})",
        muted, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TabLineSel', {{ fg = '{}', bg = '{}', bold = true }})",
        text, bg
    );
    let _ = writeln!(s, "hi('TabLineFill', {{ bg = '{}' }})", bg_panel);
    let _ = writeln!(s, "hi('Pmenu', {{ fg = '{}', bg = '{}' }})", text, bg_panel);
    let _ = writeln!(
        s,
        "hi('PmenuSel', {{ fg = '{}', bg = '{}' }})",
        text, bg_elem
    );
    let _ = writeln!(s, "hi('PmenuSbar', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(s, "hi('PmenuThumb', {{ bg = '{}' }})", border);
    let _ = writeln!(s, "hi('WinSeparator', {{ fg = '{}' }})", border_subtle);
    let _ = writeln!(s, "hi('VertSplit', {{ fg = '{}' }})", border_subtle);
    let _ = writeln!(s, "hi('SignColumn', {{ bg = '{}' }})", bg);
    let _ = writeln!(
        s,
        "hi('FoldColumn', {{ fg = '{}', bg = '{}' }})",
        border_subtle, bg
    );
    let _ = writeln!(
        s,
        "hi('Folded', {{ fg = '{}', bg = '{}' }})",
        muted, bg_elem
    );
    let _ = writeln!(s, "hi('ColorColumn', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(s, "hi('MsgArea', {{ fg = '{}' }})", text);
    let _ = writeln!(s, "hi('MoreMsg', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('ErrorMsg', {{ fg = '{}' }})", error);
    let _ = writeln!(s, "hi('WarningMsg', {{ fg = '{}' }})", warning);
    let _ = writeln!(s, "hi('Question', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('Title', {{ fg = '{}', bold = true }})", primary);
    let _ = writeln!(s, "hi('Directory', {{ fg = '{}' }})", secondary);
    let _ = writeln!(
        s,
        "hi('MatchParen', {{ fg = '{}', bold = true, underline = true }})",
        primary
    );
    let _ = writeln!(s, "hi('NonText', {{ fg = '{}' }})", border_subtle);
    let _ = writeln!(s, "hi('SpecialKey', {{ fg = '{}' }})", border_subtle);
    let _ = writeln!(s, "hi('Conceal', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('Cursor', {{ fg = '{}', bg = '{}' }})", bg, primary);
    let _ = writeln!(
        s,
        "hi('WildMenu', {{ fg = '{}', bg = '{}' }})",
        text, bg_elem
    );
    let _ = writeln!(s);

    // --- Syntax (legacy) ---
    let _ = writeln!(s, "-- Syntax");
    let _ = writeln!(s, "hi('Comment', {{ fg = '{}', italic = true }})", muted);
    let _ = writeln!(s, "hi('Constant', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('String', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('Character', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('Number', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('Boolean', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('Float', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('Identifier', {{ fg = '{}' }})", text);
    let _ = writeln!(s, "hi('Function', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('Statement', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Conditional', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Repeat', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Label', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('Operator', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('Keyword', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Exception', {{ fg = '{}' }})", error);
    let _ = writeln!(s, "hi('PreProc', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('Include', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('Define', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Macro', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('Type', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('StorageClass', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('Structure', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('Typedef', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('Special', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('SpecialChar', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('Tag', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('Delimiter', {{ fg = '{}' }})", muted);
    let _ = writeln!(
        s,
        "hi('SpecialComment', {{ fg = '{}', bold = true }})",
        muted
    );
    let _ = writeln!(s, "hi('Debug', {{ fg = '{}' }})", error);
    let _ = writeln!(
        s,
        "hi('Underlined', {{ fg = '{}', underline = true }})",
        secondary
    );
    let _ = writeln!(s, "hi('Error', {{ fg = '{}' }})", error);
    let _ = writeln!(s, "hi('Todo', {{ fg = '{}', bold = true }})", warning);
    let _ = writeln!(s);

    // --- Treesitter ---
    let _ = writeln!(s, "-- Treesitter");
    let _ = writeln!(s, "hi('@variable', {{ fg = '{}' }})", text);
    let _ = writeln!(s, "hi('@variable.builtin', {{ fg = '{}' }})", error);
    let _ = writeln!(
        s,
        "hi('@variable.parameter', {{ fg = '{}', italic = true }})",
        text
    );
    let _ = writeln!(s, "hi('@constant', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@constant.builtin', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@constant.macro', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('@module', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('@string', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('@string.escape', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@string.regex', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('@character', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('@number', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@boolean', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@float', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@function', {{ fg = '{}' }})", secondary);
    let _ = writeln!(
        s,
        "hi('@function.builtin', {{ fg = '{}', italic = true }})",
        secondary
    );
    let _ = writeln!(s, "hi('@function.macro', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('@method', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('@constructor', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('@property', {{ fg = '{}' }})", text);
    let _ = writeln!(s, "hi('@field', {{ fg = '{}' }})", text);
    let _ = writeln!(s, "hi('@parameter', {{ fg = '{}', italic = true }})", text);
    let _ = writeln!(s, "hi('@keyword', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('@keyword.function', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('@keyword.return', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('@keyword.operator', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('@operator', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('@punctuation.bracket', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('@punctuation.delimiter', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('@punctuation.special', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('@type', {{ fg = '{}' }})", info);
    let _ = writeln!(
        s,
        "hi('@type.builtin', {{ fg = '{}', italic = true }})",
        info
    );
    let _ = writeln!(s, "hi('@type.qualifier', {{ fg = '{}' }})", accent);
    let _ = writeln!(s, "hi('@tag', {{ fg = '{}' }})", secondary);
    let _ = writeln!(s, "hi('@tag.attribute', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('@tag.delimiter', {{ fg = '{}' }})", muted);
    let _ = writeln!(s, "hi('@text.literal', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('@text.reference', {{ fg = '{}' }})", secondary);
    let _ = writeln!(
        s,
        "hi('@text.title', {{ fg = '{}', bold = true }})",
        primary
    );
    let _ = writeln!(
        s,
        "hi('@text.uri', {{ fg = '{}', underline = true }})",
        secondary
    );
    let _ = writeln!(s, "hi('@text.emphasis', {{ italic = true }})");
    let _ = writeln!(s, "hi('@text.strong', {{ bold = true }})");
    let _ = writeln!(s, "hi('@comment', {{ fg = '{}', italic = true }})", muted);
    let _ = writeln!(s);

    // --- Diagnostics ---
    let _ = writeln!(s, "-- Diagnostics");
    let _ = writeln!(s, "hi('DiagnosticError', {{ fg = '{}' }})", error);
    let _ = writeln!(s, "hi('DiagnosticWarn', {{ fg = '{}' }})", warning);
    let _ = writeln!(s, "hi('DiagnosticInfo', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('DiagnosticHint', {{ fg = '{}' }})", success);
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineError', {{ sp = '{}', undercurl = true }})",
        error
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineWarn', {{ sp = '{}', undercurl = true }})",
        warning
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineInfo', {{ sp = '{}', undercurl = true }})",
        info
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineHint', {{ sp = '{}', undercurl = true }})",
        success
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextError', {{ fg = '{}', bg = '{}' }})",
        error, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextWarn', {{ fg = '{}', bg = '{}' }})",
        warning, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextInfo', {{ fg = '{}', bg = '{}' }})",
        info, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextHint', {{ fg = '{}', bg = '{}' }})",
        success, bg_elem
    );
    let _ = writeln!(s, "hi('DiagnosticSignError', {{ fg = '{}' }})", error);
    let _ = writeln!(s, "hi('DiagnosticSignWarn', {{ fg = '{}' }})", warning);
    let _ = writeln!(s, "hi('DiagnosticSignInfo', {{ fg = '{}' }})", info);
    let _ = writeln!(s, "hi('DiagnosticSignHint', {{ fg = '{}' }})", success);
    let _ = writeln!(s);

    // --- Git signs ---
    let _ = writeln!(s, "-- Git signs");
    let _ = writeln!(s, "hi('GitSignsAdd', {{ fg = '{}' }})", success);
    let _ = writeln!(s, "hi('GitSignsChange', {{ fg = '{}' }})", warning);
    let _ = writeln!(s, "hi('GitSignsDelete', {{ fg = '{}' }})", error);
    let _ = writeln!(
        s,
        "hi('DiffAdd', {{ fg = '{}', bg = '{}' }})",
        success, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffChange', {{ fg = '{}', bg = '{}' }})",
        warning, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffDelete', {{ fg = '{}', bg = '{}' }})",
        error, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffText', {{ fg = '{}', bg = '{}' }})",
        secondary, bg_elem
    );
    let _ = writeln!(s);

    // --- Telescope ---
    let _ = writeln!(s, "-- Telescope");
    let _ = writeln!(
        s,
        "hi('TelescopeNormal', {{ fg = '{}', bg = '{}' }})",
        text, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TelescopeBorder', {{ fg = '{}', bg = '{}' }})",
        border, bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TelescopeSelection', {{ fg = '{}', bg = '{}' }})",
        text, bg_elem
    );
    let _ = writeln!(
        s,
        "hi('TelescopeSelectionCaret', {{ fg = '{}', bg = '{}' }})",
        primary, bg_elem
    );
    let _ = writeln!(s, "hi('TelescopeMatching', {{ fg = '{}' }})", primary);
    let _ = writeln!(s, "hi('TelescopePromptPrefix', {{ fg = '{}' }})", primary);
    let _ = writeln!(
        s,
        "hi('TelescopePromptTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        bg, primary
    );
    let _ = writeln!(
        s,
        "hi('TelescopePreviewTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        bg, success
    );
    let _ = writeln!(
        s,
        "hi('TelescopeResultsTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        bg, secondary
    );
    let _ = writeln!(s);

    // --- LSP ---
    let _ = writeln!(s, "-- LSP");
    let _ = writeln!(s, "hi('LspReferenceText', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(s, "hi('LspReferenceRead', {{ bg = '{}' }})", bg_elem);
    let _ = writeln!(s, "hi('LspReferenceWrite', {{ bg = '{}' }})", bg_elem);

    s
}

pub fn generate_zsh_theme(colors: &ThemeColors) -> String {
    let primary = color_to_hex(colors.primary);
    let secondary = color_to_hex(colors.secondary);
    let accent = color_to_hex(colors.accent);
    let bg = color_to_hex(colors.background);
    let bg_elem = color_to_hex(colors.background_element);
    let text = color_to_hex(colors.text);
    let muted = color_to_hex(colors.text_muted);
    let border = color_to_hex(colors.border);
    let error = color_to_hex(colors.error);
    let _warning = color_to_hex(colors.warning);
    let success = color_to_hex(colors.success);
    let info = color_to_hex(colors.info);

    let (dir_r, dir_g, dir_b) = color_rgb(colors.secondary);
    let (ln_r, ln_g, ln_b) = color_rgb(colors.info);
    let (ex_r, ex_g, ex_b) = color_rgb(colors.success);
    let (ar_r, ar_g, ar_b) = color_rgb(colors.warning);
    let (media_r, media_g, media_b) = color_rgb(colors.accent);
    let (src_r, src_g, src_b) = color_rgb(colors.text);
    let (doc_r, doc_g, doc_b) = color_rgb(colors.text_muted);

    let mut s = String::with_capacity(4096);

    let _ = writeln!(
        s,
        "# OpenCode Manager ZSH Theme (auto-generated, do not edit)"
    );
    let _ = writeln!(s);

    // --- LS_COLORS ---
    let _ = writeln!(s, "# LS_COLORS");
    let _ = write!(s, "export LS_COLORS=\"");
    let _ = write!(s, "di=1;38;2;{};{};{}:", dir_r, dir_g, dir_b);
    let _ = write!(s, "ln=38;2;{};{};{}:", ln_r, ln_g, ln_b);
    let _ = write!(s, "ex=38;2;{};{};{}:", ex_r, ex_g, ex_b);
    let _ = write!(s, "*.tar=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.zip=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.gz=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.bz2=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.xz=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.7z=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.rar=38;2;{};{};{}:", ar_r, ar_g, ar_b);
    let _ = write!(s, "*.jpg=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.jpeg=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.png=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.gif=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.svg=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.mp4=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.mp3=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.wav=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.flac=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.webm=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.webp=38;2;{};{};{}:", media_r, media_g, media_b);
    let _ = write!(s, "*.rs=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.go=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.py=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.js=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.ts=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.c=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.cpp=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.h=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.lua=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.sh=38;2;{};{};{}:", src_r, src_g, src_b);
    let _ = write!(s, "*.md=38;2;{};{};{}:", doc_r, doc_g, doc_b);
    let _ = write!(s, "*.txt=38;2;{};{};{}:", doc_r, doc_g, doc_b);
    let _ = write!(s, "*.pdf=38;2;{};{};{}:", doc_r, doc_g, doc_b);
    let _ = write!(s, "*.doc=38;2;{};{};{}:", doc_r, doc_g, doc_b);
    let _ = write!(s, "*.csv=38;2;{};{};{}", doc_r, doc_g, doc_b);
    let _ = writeln!(s, "\"");
    let _ = writeln!(s);

    // --- zsh-syntax-highlighting ---
    let _ = writeln!(s, "# zsh-syntax-highlighting");
    let _ = writeln!(s, "if [[ -n ${{ZSH_HIGHLIGHT_STYLES+x}} ]]; then");
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[command]='fg={}'", secondary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[builtin]='fg={}'", secondary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[alias]='fg={}'", secondary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[path]='fg={}'", info);
    let _ = writeln!(
        s,
        "  ZSH_HIGHLIGHT_STYLES[single-quoted-argument]='fg={}'",
        success
    );
    let _ = writeln!(
        s,
        "  ZSH_HIGHLIGHT_STYLES[double-quoted-argument]='fg={}'",
        success
    );
    let _ = writeln!(
        s,
        "  ZSH_HIGHLIGHT_STYLES[dollar-quoted-argument]='fg={}'",
        success
    );
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[comment]='fg={}'", muted);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[unknown-token]='fg={}'", error);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[reserved-word]='fg={}'", accent);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[assign]='fg={}'", primary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[named-fd]='fg={}'", primary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[numeric-fd]='fg={}'", primary);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[commandseparator]='fg={}'", muted);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[redirection]='fg={}'", muted);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[globbing]='fg={}'", info);
    let _ = writeln!(s, "  ZSH_HIGHLIGHT_STYLES[default]='fg={}'", text);
    let _ = writeln!(s, "fi");
    let _ = writeln!(s);

    // --- FZF ---
    let _ = writeln!(s, "# FZF");
    let _ = writeln!(
        s,
        "export FZF_DEFAULT_OPTS=\"--color=fg:{},bg:{},hl:{},fg+:{},bg+:{},hl+:{},info:{},prompt:{},pointer:{},marker:{},spinner:{},header:{},border:{}\"",
        text, bg, primary, text, bg_elem, accent, info, primary, accent, success, secondary, muted, border
    );

    s
}

pub fn generate_gitui_theme(colors: &ThemeColors) -> String {
    let primary = color_to_hex(colors.primary);
    let secondary = color_to_hex(colors.secondary);
    let bg_elem = color_to_hex(colors.background_element);
    let text = color_to_hex(colors.text);
    let error = color_to_hex(colors.error);
    let success = color_to_hex(colors.success);
    let info = color_to_hex(colors.info);
    let muted = color_to_hex(colors.text_muted);
    let accent = color_to_hex(colors.accent);
    let warning = color_to_hex(colors.warning);

    let mut s = String::with_capacity(1024);

    let _ = writeln!(s, "// OpenCode Manager gitui theme (auto-generated)");
    let _ = writeln!(s, "(");
    let _ = writeln!(s, "    selected_tab: Some(\"{}\"),", primary);
    let _ = writeln!(s, "    command_fg: Some(\"{}\"),", text);
    let _ = writeln!(s, "    selection_bg: Some(\"{}\"),", bg_elem);
    let _ = writeln!(s, "    selection_fg: Some(\"{}\"),", text);
    let _ = writeln!(s, "    cmdbar_bg: Some(\"{}\"),", bg_elem);
    let _ = writeln!(s, "    disabled_fg: Some(\"{}\"),", muted);
    let _ = writeln!(s, "    diff_line_add: Some(\"{}\"),", success);
    let _ = writeln!(s, "    diff_line_delete: Some(\"{}\"),", error);
    let _ = writeln!(s, "    diff_file_added: Some(\"{}\"),", success);
    let _ = writeln!(s, "    diff_file_removed: Some(\"{}\"),", error);
    let _ = writeln!(s, "    diff_file_moved: Some(\"{}\"),", info);
    let _ = writeln!(s, "    diff_file_modified: Some(\"{}\"),", warning);
    let _ = writeln!(s, "    commit_hash: Some(\"{}\"),", accent);
    let _ = writeln!(s, "    commit_time: Some(\"{}\"),", muted);
    let _ = writeln!(s, "    commit_author: Some(\"{}\"),", secondary);
    let _ = writeln!(s, "    danger_fg: Some(\"{}\"),", error);
    let _ = writeln!(s, "    push_gauge_bg: Some(\"{}\"),", primary);
    let _ = writeln!(s, "    push_gauge_fg: Some(\"{}\"),", text);
    let _ = writeln!(s, "    tag_fg: Some(\"{}\"),", accent);
    let _ = writeln!(s, "    branch_fg: Some(\"{}\"),", secondary);
    let _ = writeln!(s, "    block_title_focused: Some(\"{}\"),", primary);
    let _ = writeln!(s, ")");

    s
}
