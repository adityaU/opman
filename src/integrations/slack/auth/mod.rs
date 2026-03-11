//! Slack authentication, credentials persistence, and OAuth flow.

mod oauth;

pub use oauth::run_oauth_flow;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ── Slack Auth (persisted to YAML) ──────────────────────────────────────

/// Credentials obtained through Slack OAuth + Socket Mode setup.
/// Persisted to `~/.config/opman/slack_auth.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAuth {
    /// OAuth Bot User Token (xoxb-...).
    pub bot_token: String,
    /// App-Level Token for Socket Mode (xapp-...).
    pub app_token: String,
    /// The authenticated user's Slack user ID (e.g., U01234ABC).
    pub user_id: String,
    /// OAuth Client ID (for re-auth).
    pub client_id: String,
    /// OAuth Client Secret (for re-auth).
    pub client_secret: String,
}

impl SlackAuth {
    /// Path to the auth file: `~/.config/opman/slack_auth.yaml`.
    ///
    /// Checks the platform-native config directory first (e.g.
    /// `~/Library/Application Support/opman` on macOS), then falls back to
    /// the XDG-style `~/.config/opman` directory.  Symlinked directories and
    /// files are followed transparently via `std::fs::metadata`.
    pub fn auth_path() -> Result<PathBuf> {
        let filename = "slack_auth.yaml";

        // 1. Primary: dirs::config_dir()/opman/slack_auth.yaml
        if let Some(primary) = dirs::config_dir() {
            let p = primary.join("opman").join(filename);
            if std::fs::metadata(&p).is_ok() {
                return Ok(p);
            }
        }

        // 2. Fallback: ~/.config/opman/slack_auth.yaml  (XDG / symlink-friendly)
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".config").join("opman").join(filename);
            if std::fs::metadata(&p).is_ok() {
                return Ok(p);
            }
        }

        // 3. Neither exists yet – return the primary location so callers that
        //    *create* the file use the platform-native directory.
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("opman");
        Ok(config_dir.join(filename))
    }

    /// Load credentials from disk. Returns `None` if the file doesn't exist
    /// at any of the checked locations.  Follows symlinks.
    pub fn load() -> Result<Option<Self>> {
        let path = Self::auth_path()?;
        // `metadata` follows symlinks; if it fails the file truly doesn't exist.
        if std::fs::metadata(&path).is_err() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let auth: SlackAuth = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(Some(auth))
    }

    #[allow(dead_code)] // Called from run_oauth_flow which is wired up via SlackOAuth command.
    pub fn save(&self) -> Result<()> {
        let path = Self::auth_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(self).context("Failed to serialize Slack auth")?;
        std::fs::write(&path, yaml)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        info!("Saved Slack credentials to {}", path.display());
        Ok(())
    }
}

// ── Slack Session Map (persisted to YAML) ──────────────────────────────

/// Persisted session↔thread mappings so that relay watchers survive app restarts.
/// Saved to `~/.config/opman/slack_session_map.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlackSessionMap {
    /// session_id → (channel, thread_ts) for response relay.
    pub session_threads: HashMap<String, (String, String)>,
    /// thread_ts → (project_idx, session_id).
    pub thread_sessions: HashMap<String, (usize, String)>,
    /// session_id → last relayed message count.
    pub msg_offsets: HashMap<String, usize>,
}

impl SlackSessionMap {
    /// Path to the session map file.
    pub fn map_path() -> Result<PathBuf> {
        let filename = "slack_session_map.yaml";

        // 1. Primary: dirs::config_dir()/opman/
        if let Some(primary) = dirs::config_dir() {
            let p = primary.join("opman").join(filename);
            if std::fs::metadata(&p).is_ok() {
                return Ok(p);
            }
        }

        // 2. Fallback: ~/.config/opman/
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".config").join("opman").join(filename);
            if std::fs::metadata(&p).is_ok() {
                return Ok(p);
            }
        }

        // 3. Default to primary location for creation.
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("opman");
        Ok(config_dir.join(filename))
    }

    /// Load from disk. Returns default (empty) if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::map_path()?;
        if std::fs::metadata(&path).is_err() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let map: SlackSessionMap = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(map)
    }

    /// Save to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::map_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(self).context("Failed to serialize session map")?;
        std::fs::write(&path, yaml)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        debug!("Saved Slack session map to {}", path.display());
        Ok(())
    }
}

// ── Slack Settings (in config.toml) ─────────────────────────────────────

/// User-facing Slack settings stored in `config.toml` under `[settings.slack]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackSettings {
    /// Master enable/disable for the Slack integration.
    #[serde(default)]
    pub enabled: bool,
    /// Sessions idle longer than this (minutes) are considered "free" for routing.
    #[serde(default = "default_idle_session_minutes")]
    pub idle_session_minutes: u64,
    /// How often (seconds) to batch and relay AI responses to Slack.
    #[serde(default = "default_response_batch_secs")]
    pub response_batch_secs: u64,
    /// How often (seconds) the live relay watcher polls for new messages.
    #[serde(default = "default_relay_buffer_secs")]
    pub relay_buffer_secs: u64,
}

impl Default for SlackSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            idle_session_minutes: 10,
            response_batch_secs: 30,
            relay_buffer_secs: 10,
        }
    }
}

fn default_idle_session_minutes() -> u64 {
    10
}
fn default_response_batch_secs() -> u64 {
    30
}
fn default_relay_buffer_secs() -> u64 {
    10
}
