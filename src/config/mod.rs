mod key_defaults;
mod key_parser;
mod keybindings;

pub use key_parser::{format_key_display, parse_key_combo};
pub use keybindings::KeyBindings;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Types ───────────────────────────────────────────────────────────────

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
    /// Slack integration settings.
    #[serde(default)]
    pub slack: crate::slack::SlackSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            follow_edits_in_neovim: false,
            default_terminal_command: None,
            unfocused_dim_percent: 20,
            slack: crate::slack::SlackSettings::default(),
        }
    }
}

fn default_follow_edits() -> bool {
    true
}
fn default_unfocused_dim_percent() -> u8 {
    20
}

// ── Config ──────────────────────────────────────────────────────────────

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
    /// `~/.config/opman/config.toml`
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("opman");
        Ok(config_dir.join("config.toml"))
    }

    /// Return the legacy config directory path for migration.
    fn legacy_config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("opencode-manager"))
    }

    /// Migrate from the old `~/.config/opencode-manager/` directory to
    /// `~/.config/opman/` if the old directory exists and the new one does not.
    /// Moves the entire directory (config.toml, themes, etc.) in one operation.
    fn migrate_legacy_config() {
        let new_dir = match dirs::config_dir() {
            Some(d) => d.join("opman"),
            None => return,
        };
        if new_dir.exists() {
            return; // new config already exists, nothing to migrate
        }
        if let Some(old_dir) = Self::legacy_config_dir() {
            if old_dir.exists() {
                if let Err(e) = fs::rename(&old_dir, &new_dir) {
                    // rename may fail across filesystems; fall back to copy
                    eprintln!(
                        "Note: could not rename {} -> {}: {}. Trying copy.",
                        old_dir.display(),
                        new_dir.display(),
                        e
                    );
                    if let Err(e2) = Self::copy_dir_recursive(&old_dir, &new_dir) {
                        eprintln!("Failed to migrate legacy config: {}", e2);
                    } else {
                        let _ = fs::remove_dir_all(&old_dir);
                    }
                }
            }
        }
    }

    /// Recursively copy a directory tree.
    fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let dest_path = dst.join(entry.file_name());
            if ty.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &dest_path)?;
            } else {
                fs::copy(entry.path(), &dest_path)?;
            }
        }
        Ok(())
    }

    /// Load the config from disk, or return the default if the file doesn't exist.
    /// Automatically migrates from the legacy config directory if needed.
    pub fn load() -> Result<Self> {
        Self::migrate_legacy_config();
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
