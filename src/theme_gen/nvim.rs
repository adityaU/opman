use std::fmt::Write;

use crate::theme::{color_to_hex, ThemeColors};

use super::nvim_ext;

/// Holds pre-computed hex color strings for nvim highlight generation.
pub(super) struct NvimColors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub bg: String,
    pub bg_panel: String,
    pub bg_elem: String,
    pub text: String,
    pub muted: String,
    pub border: String,
    pub border_subtle: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub info: String,
}

impl NvimColors {
    fn from_theme(colors: &ThemeColors) -> Self {
        Self {
            primary: color_to_hex(colors.primary),
            secondary: color_to_hex(colors.secondary),
            accent: color_to_hex(colors.accent),
            bg: color_to_hex(colors.background),
            bg_panel: color_to_hex(colors.background_panel),
            bg_elem: color_to_hex(colors.background_element),
            text: color_to_hex(colors.text),
            muted: color_to_hex(colors.text_muted),
            border: color_to_hex(colors.border),
            border_subtle: color_to_hex(colors.border_subtle),
            error: color_to_hex(colors.error),
            warning: color_to_hex(colors.warning),
            success: color_to_hex(colors.success),
            info: color_to_hex(colors.info),
        }
    }
}

pub fn generate_nvim_colorscheme(colors: &ThemeColors) -> String {
    let c = NvimColors::from_theme(colors);
    let mut s = String::with_capacity(16384);

    write_nvim_header(&mut s);
    write_nvim_editor_ui(&mut s, &c);
    write_nvim_syntax(&mut s, &c);
    nvim_ext::write_nvim_treesitter(&mut s, &c);
    nvim_ext::write_nvim_diagnostics(&mut s, &c);
    nvim_ext::write_nvim_git_signs(&mut s, &c);
    nvim_ext::write_nvim_telescope(&mut s, &c);
    nvim_ext::write_nvim_lsp(&mut s, &c);

    s
}

fn write_nvim_header(s: &mut String) {
    let _ = writeln!(s, "-- OpenCode Manager Theme (auto-generated, do not edit)");
    let _ = writeln!(s, "vim.cmd('highlight clear')");
    let _ = writeln!(s, "vim.o.termguicolors = true");
    let _ = writeln!(s, "vim.g.colors_name = 'opencode'");
    let _ = writeln!(s);
    let _ = writeln!(s, "local hi = function(group, opts)");
    let _ = writeln!(s, "  vim.api.nvim_set_hl(0, group, opts)");
    let _ = writeln!(s, "end");
    let _ = writeln!(s);
}

fn write_nvim_editor_ui(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Editor UI");
    let _ = writeln!(s, "hi('Normal', {{ fg = '{}', bg = '{}' }})", c.text, c.bg);
    let _ = writeln!(
        s,
        "hi('NormalFloat', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('FloatBorder', {{ fg = '{}', bg = '{}' }})",
        c.border, c.bg_panel
    );
    let _ = writeln!(s, "hi('CursorLine', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(
        s,
        "hi('CursorLineNr', {{ fg = '{}', bold = true }})",
        c.primary
    );
    let _ = writeln!(s, "hi('LineNr', {{ fg = '{}' }})", c.border_subtle);
    let _ = writeln!(s, "hi('Visual', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(
        s,
        "hi('Search', {{ fg = '{}', bg = '{}' }})",
        c.bg, c.warning
    );
    let _ = writeln!(
        s,
        "hi('IncSearch', {{ fg = '{}', bg = '{}' }})",
        c.bg, c.primary
    );
    let _ = writeln!(
        s,
        "hi('StatusLine', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('StatusLineNC', {{ fg = '{}', bg = '{}' }})",
        c.muted, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TabLine', {{ fg = '{}', bg = '{}' }})",
        c.muted, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TabLineSel', {{ fg = '{}', bg = '{}', bold = true }})",
        c.text, c.bg
    );
    let _ = writeln!(s, "hi('TabLineFill', {{ bg = '{}' }})", c.bg_panel);
    let _ = writeln!(
        s,
        "hi('Pmenu', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('PmenuSel', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_elem
    );
    let _ = writeln!(s, "hi('PmenuSbar', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(s, "hi('PmenuThumb', {{ bg = '{}' }})", c.border);
    let _ = writeln!(s, "hi('WinSeparator', {{ fg = '{}' }})", c.border_subtle);
    let _ = writeln!(s, "hi('VertSplit', {{ fg = '{}' }})", c.border_subtle);
    let _ = writeln!(s, "hi('SignColumn', {{ bg = '{}' }})", c.bg);
    let _ = writeln!(
        s,
        "hi('FoldColumn', {{ fg = '{}', bg = '{}' }})",
        c.border_subtle, c.bg
    );
    let _ = writeln!(
        s,
        "hi('Folded', {{ fg = '{}', bg = '{}' }})",
        c.muted, c.bg_elem
    );
    let _ = writeln!(s, "hi('ColorColumn', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(s, "hi('MsgArea', {{ fg = '{}' }})", c.text);
    let _ = writeln!(s, "hi('MoreMsg', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('ErrorMsg', {{ fg = '{}' }})", c.error);
    let _ = writeln!(s, "hi('WarningMsg', {{ fg = '{}' }})", c.warning);
    let _ = writeln!(s, "hi('Question', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('Title', {{ fg = '{}', bold = true }})", c.primary);
    let _ = writeln!(s, "hi('Directory', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(
        s,
        "hi('MatchParen', {{ fg = '{}', bold = true, underline = true }})",
        c.primary
    );
    let _ = writeln!(s, "hi('NonText', {{ fg = '{}' }})", c.border_subtle);
    let _ = writeln!(s, "hi('SpecialKey', {{ fg = '{}' }})", c.border_subtle);
    let _ = writeln!(s, "hi('Conceal', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(
        s,
        "hi('Cursor', {{ fg = '{}', bg = '{}' }})",
        c.bg, c.primary
    );
    let _ = writeln!(
        s,
        "hi('WildMenu', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_elem
    );
    let _ = writeln!(s);
}

fn write_nvim_syntax(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Syntax");
    let _ = writeln!(s, "hi('Comment', {{ fg = '{}', italic = true }})", c.muted);
    let _ = writeln!(s, "hi('Constant', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('String', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('Character', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('Number', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('Boolean', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('Float', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('Identifier', {{ fg = '{}' }})", c.text);
    let _ = writeln!(s, "hi('Function', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('Statement', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Conditional', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Repeat', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Label', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('Operator', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(s, "hi('Keyword', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Exception', {{ fg = '{}' }})", c.error);
    let _ = writeln!(s, "hi('PreProc', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('Include', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('Define', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Macro', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('Type', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('StorageClass', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('Structure', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('Typedef', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('Special', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('SpecialChar', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('Tag', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('Delimiter', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(
        s,
        "hi('SpecialComment', {{ fg = '{}', bold = true }})",
        c.muted
    );
    let _ = writeln!(s, "hi('Debug', {{ fg = '{}' }})", c.error);
    let _ = writeln!(
        s,
        "hi('Underlined', {{ fg = '{}', underline = true }})",
        c.secondary
    );
    let _ = writeln!(s, "hi('Error', {{ fg = '{}' }})", c.error);
    let _ = writeln!(s, "hi('Todo', {{ fg = '{}', bold = true }})", c.warning);
    let _ = writeln!(s);
}
