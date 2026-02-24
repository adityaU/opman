use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A single project entry in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    /// Display name for the project.
    pub name: String,
    /// Absolute path to the project directory.
    pub path: String,
    /// Optional command to run in the integrated terminal for this project.
    /// If not set, falls back to the global default_terminal_command, then to $SHELL.
    #[serde(default)]
    pub terminal_command: Option<String>,
}

/// Settings toggled via the config panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// When enabled, file edits from the active opencode session are streamed
    /// as running diffs to the embedded neovim instance.
    #[serde(default = "default_follow_edits")]
    pub follow_edits_in_neovim: bool,
    /// Default command to run in the integrated terminal (e.g., "fish", "zsh --login").
    /// If not set, uses $SHELL environment variable, falling back to "/bin/bash".
    #[serde(default)]
    pub default_terminal_command: Option<String>,
    /// How much to dim unfocused panels, as a percentage (0–100).
    /// 0 = no dimming, 100 = fully black.  Default is 20.
    #[serde(default = "default_unfocused_dim_percent")]
    pub unfocused_dim_percent: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            follow_edits_in_neovim: false,
            default_terminal_command: None,
            unfocused_dim_percent: 20,
        }
    }
}

// ── Keybinding configuration ────────────────────────────────────────────

/// User-configurable keybindings.
///
/// Each field is a key-combo string such as `"ctrl+q"`, `"space"`, `"a"`, `"ctrl+shift+s"`.
/// Supported modifiers: `ctrl`, `alt`, `shift`. Separator: `+`.
/// Special key names: `space`, `enter`, `esc`, `tab`, `backspace`, `up`, `down`, `left`, `right`,
/// `home`, `end`, `pageup`, `pagedown`, `delete`, `insert`, `f1`..`f12`, `/`, `.`, `?`, `:`.
///
/// The "leader" key opens the which-key prefix menu. Sub-bindings under the leader
/// (e.g. `leader_terminal`) are single keys pressed *after* the leader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    // ── Global ──────────────────────────────────────────────────────
    #[serde(default = "default_quit")]
    pub quit: String,
    #[serde(default = "default_cheatsheet")]
    pub cheatsheet: String,
    #[serde(default = "default_cheatsheet_alt")]
    pub cheatsheet_alt: String,
    #[serde(default = "default_insert_i")]
    pub insert_i: String,
    #[serde(default = "default_insert_a")]
    pub insert_a: String,
    #[serde(default = "default_insert_enter")]
    pub insert_enter: String,
    #[serde(default = "default_command_mode")]
    pub command_mode: String,
    #[serde(default = "default_resize_mode")]
    pub resize_mode: String,

    // ── Navigation (normal + insert) ────────────────────────────────
    #[serde(default = "default_nav_left")]
    pub nav_left: String,
    #[serde(default = "default_nav_right")]
    pub nav_right: String,
    #[serde(default = "default_nav_down")]
    pub nav_down: String,
    #[serde(default = "default_nav_up")]
    pub nav_up: String,

    // ── Direct shortcuts ────────────────────────────────────────────
    #[serde(default = "default_toggle_sidebar")]
    pub toggle_sidebar: String,
    #[serde(default = "default_toggle_terminal")]
    pub toggle_terminal: String,
    #[serde(default = "default_toggle_neovim")]
    pub toggle_neovim: String,
    #[serde(default = "default_toggle_git")]
    pub toggle_git: String,
    #[serde(default = "default_swap_panel")]
    pub swap_panel: String,
    #[serde(default = "default_swap_panel_alt")]
    pub swap_panel_alt: String,

    // ── Leader key ──────────────────────────────────────────────────
    #[serde(default = "default_leader")]
    pub leader: String,

    // ── Leader sub-bindings (pressed after leader) ──────────────────
    #[serde(default = "default_leader_terminal")]
    pub leader_terminal: String,
    #[serde(default = "default_leader_git")]
    pub leader_git: String,
    #[serde(default = "default_leader_neovim")]
    pub leader_neovim: String,
    #[serde(default = "default_leader_zen")]
    pub leader_zen: String,
    #[serde(default = "default_zen_toggle")]
    pub zen_toggle: String,
    #[serde(default = "default_zen_terminal")]
    pub zen_terminal: String,
    #[serde(default = "default_zen_opencode")]
    pub zen_opencode: String,
    #[serde(default = "default_zen_neovim")]
    pub zen_neovim: String,
    #[serde(default = "default_zen_git")]
    pub zen_git: String,
    #[serde(default = "default_leader_config")]
    pub leader_config: String,
    #[serde(default = "default_leader_search")]
    pub leader_search: String,
    #[serde(default = "default_leader_quit")]
    pub leader_quit: String,
    #[serde(default = "default_leader_todo")]
    pub leader_todo: String,
    #[serde(default = "default_leader_context")]
    pub leader_context: String,

    // ── Leader → Terminal sub-bindings ──────────────────────────────
    #[serde(default = "default_terminal_toggle")]
    pub terminal_toggle: String,
    #[serde(default = "default_terminal_new_tab")]
    pub terminal_new_tab: String,
    #[serde(default = "default_terminal_next_tab")]
    pub terminal_next_tab: String,
    #[serde(default = "default_terminal_prev_tab")]
    pub terminal_prev_tab: String,
    #[serde(default = "default_terminal_close_tab")]
    pub terminal_close_tab: String,
    #[serde(default = "default_terminal_search")]
    pub terminal_search: String,

    // ── Leader → Project sub-bindings ───────────────────────────────
    #[serde(default = "default_leader_project")]
    pub leader_project: String,
    #[serde(default = "default_project_picker")]
    pub project_picker: String,
    #[serde(default = "default_project_add")]
    pub project_add: String,
    #[serde(default = "default_project_sessions")]
    pub project_sessions: String,

    // ── Leader → Window sub-bindings ────────────────────────────────
    #[serde(default = "default_leader_window")]
    pub leader_window: String,
    #[serde(default = "default_window_left")]
    pub window_left: String,
    #[serde(default = "default_window_right")]
    pub window_right: String,
    #[serde(default = "default_window_down")]
    pub window_down: String,
    #[serde(default = "default_window_up")]
    pub window_up: String,
    #[serde(default = "default_window_float")]
    pub window_float: String,
    #[serde(default = "default_window_popout")]
    pub window_popout: String,
    #[serde(default = "default_window_swap")]
    pub window_swap: String,
    #[serde(default = "default_window_sidebar")]
    pub window_sidebar: String,

    // ── Resize mode ─────────────────────────────────────────────────
    #[serde(default = "default_resize_left")]
    pub resize_left: String,
    #[serde(default = "default_resize_right")]
    pub resize_right: String,
    #[serde(default = "default_resize_down")]
    pub resize_down: String,
    #[serde(default = "default_resize_up")]
    pub resize_up: String,
}

// ── Default value functions for serde ───────────────────────────────────

fn default_quit() -> String {
    "ctrl+q".into()
}
fn default_cheatsheet() -> String {
    "ctrl+/".into()
}
fn default_cheatsheet_alt() -> String {
    "?".into()
}
fn default_insert_i() -> String {
    "i".into()
}
fn default_insert_a() -> String {
    "a".into()
}
fn default_insert_enter() -> String {
    "enter".into()
}
fn default_command_mode() -> String {
    ":".into()
}
fn default_resize_mode() -> String {
    "r".into()
}
fn default_nav_left() -> String {
    "ctrl+h".into()
}
fn default_nav_right() -> String {
    "ctrl+l".into()
}
fn default_nav_down() -> String {
    "ctrl+j".into()
}
fn default_nav_up() -> String {
    "ctrl+k".into()
}
fn default_toggle_sidebar() -> String {
    "ctrl+b".into()
}
fn default_toggle_terminal() -> String {
    "ctrl+t".into()
}
fn default_toggle_neovim() -> String {
    "ctrl+n".into()
}
fn default_toggle_git() -> String {
    "ctrl+g".into()
}
fn default_swap_panel() -> String {
    "ctrl+.".into()
}
fn default_swap_panel_alt() -> String {
    "ctrl+shift+s".into()
}
fn default_leader() -> String {
    "space".into()
}
fn default_leader_terminal() -> String {
    "t".into()
}
fn default_leader_git() -> String {
    "g".into()
}
fn default_follow_edits() -> bool {
    true
}
fn default_unfocused_dim_percent() -> u8 {
    20
}
fn default_leader_neovim() -> String {
    "n".into()
}
fn default_leader_zen() -> String {
    "z".into()
}
fn default_zen_toggle() -> String {
    "z".into()
}
fn default_zen_terminal() -> String {
    "t".into()
}
fn default_zen_opencode() -> String {
    "o".into()
}
fn default_zen_neovim() -> String {
    "n".into()
}
fn default_zen_git() -> String {
    "g".into()
}
fn default_leader_config() -> String {
    "c".into()
}
fn default_leader_search() -> String {
    "/".into()
}
fn default_leader_quit() -> String {
    "q".into()
}
fn default_leader_todo() -> String {
    "d".into()
}
fn default_leader_context() -> String {
    "i".into()
}
fn default_leader_project() -> String {
    "p".into()
}
fn default_project_picker() -> String {
    "p".into()
}
fn default_project_add() -> String {
    "a".into()
}
fn default_project_sessions() -> String {
    "s".into()
}
fn default_leader_window() -> String {
    "w".into()
}
fn default_window_left() -> String {
    "h".into()
}
fn default_window_right() -> String {
    "l".into()
}
fn default_window_down() -> String {
    "j".into()
}
fn default_window_up() -> String {
    "k".into()
}
fn default_window_float() -> String {
    "f".into()
}
fn default_window_popout() -> String {
    "w".into()
}
fn default_window_swap() -> String {
    "s".into()
}
fn default_window_sidebar() -> String {
    "b".into()
}
fn default_resize_left() -> String {
    "h".into()
}
fn default_resize_right() -> String {
    "l".into()
}
fn default_resize_down() -> String {
    "j".into()
}
fn default_resize_up() -> String {
    "k".into()
}
fn default_terminal_toggle() -> String {
    "t".into()
}
fn default_terminal_new_tab() -> String {
    "n".into()
}
fn default_terminal_next_tab() -> String {
    "]".into()
}
fn default_terminal_prev_tab() -> String {
    "[".into()
}
fn default_terminal_close_tab() -> String {
    "x".into()
}
fn default_terminal_search() -> String {
    "f".into()
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            quit: default_quit(),
            cheatsheet: default_cheatsheet(),
            cheatsheet_alt: default_cheatsheet_alt(),
            insert_i: default_insert_i(),
            insert_a: default_insert_a(),
            insert_enter: default_insert_enter(),
            command_mode: default_command_mode(),
            resize_mode: default_resize_mode(),
            nav_left: default_nav_left(),
            nav_right: default_nav_right(),
            nav_down: default_nav_down(),
            nav_up: default_nav_up(),
            toggle_sidebar: default_toggle_sidebar(),
            toggle_terminal: default_toggle_terminal(),
            toggle_neovim: default_toggle_neovim(),
            toggle_git: default_toggle_git(),
            swap_panel: default_swap_panel(),
            swap_panel_alt: default_swap_panel_alt(),
            leader: default_leader(),
            leader_terminal: default_leader_terminal(),
            leader_git: default_leader_git(),
            leader_neovim: default_leader_neovim(),
            leader_zen: default_leader_zen(),
            zen_toggle: default_zen_toggle(),
            zen_terminal: default_zen_terminal(),
            zen_opencode: default_zen_opencode(),
            zen_neovim: default_zen_neovim(),
            zen_git: default_zen_git(),
            leader_config: default_leader_config(),
            leader_search: default_leader_search(),
            leader_quit: default_leader_quit(),
            leader_todo: default_leader_todo(),
            leader_context: default_leader_context(),
            terminal_toggle: default_terminal_toggle(),
            terminal_new_tab: default_terminal_new_tab(),
            terminal_next_tab: default_terminal_next_tab(),
            terminal_prev_tab: default_terminal_prev_tab(),
            terminal_close_tab: default_terminal_close_tab(),
            terminal_search: default_terminal_search(),
            leader_project: default_leader_project(),
            project_picker: default_project_picker(),
            project_add: default_project_add(),
            project_sessions: default_project_sessions(),
            leader_window: default_leader_window(),
            window_left: default_window_left(),
            window_right: default_window_right(),
            window_down: default_window_down(),
            window_up: default_window_up(),
            window_float: default_window_float(),
            window_popout: default_window_popout(),
            window_swap: default_window_swap(),
            window_sidebar: default_window_sidebar(),
            resize_left: default_resize_left(),
            resize_right: default_resize_right(),
            resize_down: default_resize_down(),
            resize_up: default_resize_up(),
        }
    }
}

// ── Key string parser ───────────────────────────────────────────────────

use crate::which_key::KeyCombo;
use crossterm::event::{KeyCode, KeyModifiers};

/// Parse a human-readable key string into a `KeyCombo`.
///
/// Examples: `"ctrl+q"`, `"ctrl+shift+s"`, `"space"`, `"a"`, `"f1"`, `"ctrl+/"`.
pub fn parse_key_combo(s: &str) -> Result<KeyCombo> {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();

    let mut modifiers = KeyModifiers::NONE;
    let key_part;

    if parts.len() == 1 {
        key_part = parts[0];
    } else {
        // All parts except the last are modifiers
        for &m in &parts[..parts.len() - 1] {
            match m {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" | "opt" | "option" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                other => anyhow::bail!("Unknown modifier '{other}' in key string '{s}'"),
            }
        }
        key_part = parts[parts.len() - 1];
    }

    let code = match key_part {
        "space" | "spc" => KeyCode::Char(' '),
        "enter" | "return" | "cr" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" | "pgdown" => KeyCode::PageDown,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        // Single character keys
        k if k.len() == 1 => {
            let ch = k.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) && ch.is_ascii_alphabetic() {
                // Shift+letter: crossterm sends uppercase char with SHIFT modifier
                KeyCode::Char(ch.to_ascii_uppercase())
            } else {
                KeyCode::Char(ch)
            }
        }
        other => anyhow::bail!("Unknown key name '{other}' in key string '{s}'"),
    };

    Ok(KeyCombo::new(modifiers, code))
}

/// Format a key-combo string back to a display label, e.g. `"Ctrl+Q"`.
pub fn format_key_display(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();
    let mut out = Vec::new();
    for p in &parts {
        match *p {
            "ctrl" | "control" => out.push("Ctrl".to_string()),
            "alt" | "meta" | "opt" | "option" => out.push("Alt".to_string()),
            "shift" => out.push("Shift".to_string()),
            "space" | "spc" => out.push("Space".to_string()),
            "enter" | "return" | "cr" => out.push("Enter".to_string()),
            "esc" | "escape" => out.push("Esc".to_string()),
            "tab" => out.push("Tab".to_string()),
            "backspace" | "bs" => out.push("BS".to_string()),
            other => {
                if other.len() == 1 {
                    out.push(other.to_uppercase());
                } else {
                    // F-keys, special names
                    let mut c = other.chars();
                    let first = c.next().unwrap().to_uppercase().to_string();
                    out.push(format!("{first}{}", c.as_str()));
                }
            }
        }
    }
    out.join("+")
}

/// Top-level configuration persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// List of registered projects.
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
    /// User settings.
    #[serde(default)]
    pub settings: Settings,
    /// Customisable keybindings.
    #[serde(default)]
    pub keybindings: KeyBindings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            settings: Settings::default(),
            keybindings: KeyBindings::default(),
        }
    }
}

impl Config {
    /// Return the path to the config file:
    /// `~/.config/opencode-manager/config.toml`
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("opencode-manager");
        Ok(config_dir.join("config.toml"))
    }

    /// Load the config from disk, or return the default if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config from {}", path.display()))?;
        Ok(config)
    }

    /// Save the current config to disk, creating parent directories as needed.
    /// Project paths are canonicalized (symlinks resolved) before writing.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory {}", parent.display())
            })?;
        }
        let mut config_to_save = self.clone();
        for entry in &mut config_to_save.projects {
            if let Ok(canonical) = fs::canonicalize(&entry.path) {
                entry.path = canonical.to_string_lossy().to_string();
            }
        }
        let contents =
            toml::to_string_pretty(&config_to_save).context("Failed to serialize config")?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }
}
