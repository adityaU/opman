use std::fmt::Write;

use super::nvim::NvimColors;

pub(super) fn write_nvim_treesitter(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Treesitter");
    let _ = writeln!(s, "hi('@variable', {{ fg = '{}' }})", c.text);
    let _ = writeln!(s, "hi('@variable.builtin', {{ fg = '{}' }})", c.error);
    let _ = writeln!(
        s,
        "hi('@variable.parameter', {{ fg = '{}', italic = true }})",
        c.text
    );
    let _ = writeln!(s, "hi('@constant', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@constant.builtin', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@constant.macro', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('@module', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('@string', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('@string.escape', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@string.regex', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('@character', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('@number', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@boolean', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@float', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@function', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(
        s,
        "hi('@function.builtin', {{ fg = '{}', italic = true }})",
        c.secondary
    );
    let _ = writeln!(s, "hi('@function.macro', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('@method', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('@constructor', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('@property', {{ fg = '{}' }})", c.text);
    let _ = writeln!(s, "hi('@field', {{ fg = '{}' }})", c.text);
    let _ = writeln!(
        s,
        "hi('@parameter', {{ fg = '{}', italic = true }})",
        c.text
    );
    let _ = writeln!(s, "hi('@keyword', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('@keyword.function', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('@keyword.return', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('@keyword.operator', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('@operator', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(s, "hi('@punctuation.bracket', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(s, "hi('@punctuation.delimiter', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(s, "hi('@punctuation.special', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('@type', {{ fg = '{}' }})", c.info);
    let _ = writeln!(
        s,
        "hi('@type.builtin', {{ fg = '{}', italic = true }})",
        c.info
    );
    let _ = writeln!(s, "hi('@type.qualifier', {{ fg = '{}' }})", c.accent);
    let _ = writeln!(s, "hi('@tag', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(s, "hi('@tag.attribute', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('@tag.delimiter', {{ fg = '{}' }})", c.muted);
    let _ = writeln!(s, "hi('@text.literal', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('@text.reference', {{ fg = '{}' }})", c.secondary);
    let _ = writeln!(
        s,
        "hi('@text.title', {{ fg = '{}', bold = true }})",
        c.primary
    );
    let _ = writeln!(
        s,
        "hi('@text.uri', {{ fg = '{}', underline = true }})",
        c.secondary
    );
    let _ = writeln!(s, "hi('@text.emphasis', {{ italic = true }})");
    let _ = writeln!(s, "hi('@text.strong', {{ bold = true }})");
    let _ = writeln!(s, "hi('@comment', {{ fg = '{}', italic = true }})", c.muted);
    let _ = writeln!(s);
}

pub(super) fn write_nvim_diagnostics(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Diagnostics");
    let _ = writeln!(s, "hi('DiagnosticError', {{ fg = '{}' }})", c.error);
    let _ = writeln!(s, "hi('DiagnosticWarn', {{ fg = '{}' }})", c.warning);
    let _ = writeln!(s, "hi('DiagnosticInfo', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('DiagnosticHint', {{ fg = '{}' }})", c.success);
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineError', {{ sp = '{}', undercurl = true }})",
        c.error
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineWarn', {{ sp = '{}', undercurl = true }})",
        c.warning
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineInfo', {{ sp = '{}', undercurl = true }})",
        c.info
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticUnderlineHint', {{ sp = '{}', undercurl = true }})",
        c.success
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextError', {{ fg = '{}', bg = '{}' }})",
        c.error, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextWarn', {{ fg = '{}', bg = '{}' }})",
        c.warning, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextInfo', {{ fg = '{}', bg = '{}' }})",
        c.info, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiagnosticVirtualTextHint', {{ fg = '{}', bg = '{}' }})",
        c.success, c.bg_elem
    );
    let _ = writeln!(s, "hi('DiagnosticSignError', {{ fg = '{}' }})", c.error);
    let _ = writeln!(s, "hi('DiagnosticSignWarn', {{ fg = '{}' }})", c.warning);
    let _ = writeln!(s, "hi('DiagnosticSignInfo', {{ fg = '{}' }})", c.info);
    let _ = writeln!(s, "hi('DiagnosticSignHint', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s);
}

pub(super) fn write_nvim_git_signs(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Git signs");
    let _ = writeln!(s, "hi('GitSignsAdd', {{ fg = '{}' }})", c.success);
    let _ = writeln!(s, "hi('GitSignsChange', {{ fg = '{}' }})", c.warning);
    let _ = writeln!(s, "hi('GitSignsDelete', {{ fg = '{}' }})", c.error);
    let _ = writeln!(
        s,
        "hi('DiffAdd', {{ fg = '{}', bg = '{}' }})",
        c.success, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffChange', {{ fg = '{}', bg = '{}' }})",
        c.warning, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffDelete', {{ fg = '{}', bg = '{}' }})",
        c.error, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('DiffText', {{ fg = '{}', bg = '{}' }})",
        c.secondary, c.bg_elem
    );
    let _ = writeln!(s);
}

pub(super) fn write_nvim_telescope(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- Telescope");
    let _ = writeln!(
        s,
        "hi('TelescopeNormal', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TelescopeBorder', {{ fg = '{}', bg = '{}' }})",
        c.border, c.bg_panel
    );
    let _ = writeln!(
        s,
        "hi('TelescopeSelection', {{ fg = '{}', bg = '{}' }})",
        c.text, c.bg_elem
    );
    let _ = writeln!(
        s,
        "hi('TelescopeSelectionCaret', {{ fg = '{}', bg = '{}' }})",
        c.primary, c.bg_elem
    );
    let _ = writeln!(s, "hi('TelescopeMatching', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(s, "hi('TelescopePromptPrefix', {{ fg = '{}' }})", c.primary);
    let _ = writeln!(
        s,
        "hi('TelescopePromptTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        c.bg, c.primary
    );
    let _ = writeln!(
        s,
        "hi('TelescopePreviewTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        c.bg, c.success
    );
    let _ = writeln!(
        s,
        "hi('TelescopeResultsTitle', {{ fg = '{}', bg = '{}', bold = true }})",
        c.bg, c.secondary
    );
    let _ = writeln!(s);
}

pub(super) fn write_nvim_lsp(s: &mut String, c: &NvimColors) {
    let _ = writeln!(s, "-- LSP");
    let _ = writeln!(s, "hi('LspReferenceText', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(s, "hi('LspReferenceRead', {{ bg = '{}' }})", c.bg_elem);
    let _ = writeln!(s, "hi('LspReferenceWrite', {{ bg = '{}' }})", c.bg_elem);
}
