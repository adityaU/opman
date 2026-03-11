use ratatui::style::Color;

use super::colors::color_to_hex;
use super::ThemeColors;

impl ThemeColors {
    /// Return environment variables that hint PTY programs about the current
    /// theme so that tools like neovim, gitui, bat, fzf, etc. can pick
    /// matching colors automatically.
    pub fn pty_env_vars(&self) -> Vec<(String, String)> {
        let bg = color_to_hex(self.background);
        let fg = color_to_hex(self.text);
        let bg_panel = color_to_hex(self.background_panel);
        let bg_element = color_to_hex(self.background_element);
        let border = color_to_hex(self.border);
        let primary = color_to_hex(self.primary);
        let secondary = color_to_hex(self.secondary);
        let accent = color_to_hex(self.accent);
        let error = color_to_hex(self.error);
        let warning = color_to_hex(self.warning);
        let success = color_to_hex(self.success);
        let info = color_to_hex(self.info);
        let muted = color_to_hex(self.text_muted);

        let is_dark = match self.background {
            Color::Rgb(r, g, b) => ((r as u16 + g as u16 + b as u16) / 3) < 128,
            _ => true,
        };

        let mut vars: Vec<(String, String)> = vec![
            ("COLORFGBG".into(), format!("{};{}", if is_dark { "15" } else { "0" }, if is_dark { "0" } else { "15" })),
            ("BACKGROUND".into(), bg.clone()),
            ("FOREGROUND".into(), fg.clone()),
            ("NVIM_TUI_ENABLE_TRUE_COLOR".into(), "1".into()),
            ("BAT_THEME".into(), if is_dark { "base16" } else { "GitHub" }.into()),
            ("FZF_DEFAULT_OPTS".into(), format!(
                "--color=bg:{},fg:{},hl:{},bg+:{},fg+:{},hl+:{},info:{},prompt:{},pointer:{},marker:{},spinner:{},header:{},border:{}",
                bg, fg, primary, bg_element, fg, accent, info, primary, accent, success, secondary, muted, border
            )),
            ("LG_ACCENT_COLOR".into(), primary.clone()),
            ("OPENCODE_BG".into(), bg),
            ("OPENCODE_FG".into(), fg),
            ("OPENCODE_BG_PANEL".into(), bg_panel),
            ("OPENCODE_BG_ELEMENT".into(), bg_element),
            ("OPENCODE_BORDER".into(), border),
            ("OPENCODE_PRIMARY".into(), primary),
            ("OPENCODE_SECONDARY".into(), secondary),
            ("OPENCODE_ACCENT".into(), accent),
            ("OPENCODE_ERROR".into(), error),
            ("OPENCODE_WARNING".into(), warning),
            ("OPENCODE_SUCCESS".into(), success),
            ("OPENCODE_INFO".into(), info),
            ("OPENCODE_MUTED".into(), muted),
        ];

        if is_dark {
            vars.push(("VIM_BACKGROUND".into(), "dark".into()));
        } else {
            vars.push(("VIM_BACKGROUND".into(), "light".into()));
        }

        vars
    }
}
