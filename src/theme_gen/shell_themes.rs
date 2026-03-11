use std::fmt::Write;

use crate::theme::{color_to_hex, ThemeColors};

use super::color_rgb;

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
