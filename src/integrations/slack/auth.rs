//! Slack authentication, credentials persistence, and OAuth flow.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

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

// ── Slack OAuth Flow ────────────────────────────────────────────────────

#[allow(dead_code)] // Used in run_oauth_flow.
const BOT_SCOPES: &str = "chat:write,im:history,im:read,im:write,users:read";

/// Fixed port for the OAuth callback server so the redirect URI is predictable.
/// Configure `http://127.0.0.1:17380/slack/oauth/callback` in your Slack app.
const OAUTH_CALLBACK_PORT: u16 = 17380;

/// Run the OAuth flow: open browser → local callback server → exchange code → save.
/// Returns the populated `SlackAuth` on success.
#[allow(dead_code)] // Wired via SlackOAuth command action.
pub async fn run_oauth_flow(client_id: &str, client_secret: &str) -> Result<SlackAuth> {
    // Bind the fixed callback port.
    let listener = TcpListener::bind(format!("127.0.0.1:{}", OAUTH_CALLBACK_PORT))
        .await
        .with_context(|| {
            format!(
                "Port {} already in use – is another OAuth flow running?",
                OAUTH_CALLBACK_PORT
            )
        })?;
    let redirect_uri = format!(
        "http://127.0.0.1:{}/slack/oauth/callback",
        OAUTH_CALLBACK_PORT
    );

    // Generate a random state parameter for CSRF protection.
    let state: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect()
    };

    let auth_url = format!(
        "https://slack.com/oauth/v2/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
        client_id,
        BOT_SCOPES,
        urlencoding(&redirect_uri),
        state,
    );

    info!("Opening Slack OAuth URL in browser...");
    if let Err(e) = open::that(&auth_url) {
        warn!(
            "Could not open browser automatically: {}. Please visit: {}",
            e, auth_url
        );
    }

    // Wait for the callback.
    let code = wait_for_oauth_callback(listener, &state).await?;

    // Exchange the code for tokens.
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/oauth.v2.access")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ])
        .send()
        .await
        .context("Failed to exchange OAuth code")?;

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse OAuth response")?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("Slack OAuth failed: {}", err);
    }

    let bot_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .context("Missing access_token in OAuth response")?
        .to_string();

    // Get the authenticated user's ID.
    let user_id = fetch_auth_user_id(&client, &bot_token).await?;

    // Prompt user for app-level token (Socket Mode requires manual creation in Slack UI).
    // For now, we'll need the user to provide this. Store empty and let them fill it in.
    info!("OAuth complete. Bot token obtained. User ID: {}", user_id);
    info!("NOTE: You must create an App-Level Token in your Slack app settings");
    info!("      (Settings → Basic Information → App-Level Tokens)");
    info!(
        "      with the `connections:write` scope, then add it to slack_auth.yaml as `app_token`."
    );

    let auth = SlackAuth {
        bot_token,
        app_token: String::new(), // User must fill this in manually
        user_id,
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    auth.save()?;

    Ok(auth)
}

/// Wait for the OAuth callback on the local TCP listener.
/// Parses the `code` and `state` query parameters from the GET request.
#[allow(dead_code)] // Called from run_oauth_flow.
async fn wait_for_oauth_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    info!(
        "Waiting for OAuth callback on {}...",
        listener.local_addr()?
    );

    let (mut stream, _addr) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .context("OAuth callback timed out (5 minutes)")?
        .context("Failed to accept OAuth callback connection")?;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the first line: GET /slack/oauth/callback?code=...&state=... HTTP/1.1
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    let query = if let Some(q) = path.split('?').nth(1) {
        q
    } else {
        // Send error response.
        let resp = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<h1>Missing query parameters</h1>";
        stream.write_all(resp.as_bytes()).await.ok();
        anyhow::bail!("OAuth callback missing query parameters");
    };

    let params: HashMap<&str, &str> = query
        .split('&')
        .filter_map(|p| {
            let mut parts = p.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();

    let code = params.get("code").context("Missing 'code' parameter")?;
    let recv_state = params.get("state").context("Missing 'state' parameter")?;

    if *recv_state != expected_state {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\n\r\n<h1>State mismatch — possible CSRF</h1>";
        stream.write_all(resp.as_bytes()).await.ok();
        anyhow::bail!("OAuth state mismatch");
    }

    // Send success response.
    let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body style='font-family:sans-serif;text-align:center;padding:40px'>\
        <h1>&#10004; Slack Connected!</h1>\
        <p>You can close this tab and return to opman.</p>\
        </body></html>";
    stream.write_all(resp.as_bytes()).await.ok();

    Ok(code.to_string())
}

/// Fetch the authenticated user's Slack ID using auth.test.
#[allow(dead_code)] // Called from run_oauth_flow.
async fn fetch_auth_user_id(client: &reqwest::Client, bot_token: &str) -> Result<String> {
    let resp = client
        .post("https://slack.com/api/auth.test")
        .bearer_auth(bot_token)
        .send()
        .await
        .context("Failed to call auth.test")?;

    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("auth.test failed: {}", err);
    }

    body.get("user_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing user_id in auth.test response")
}

/// Minimal percent-encoding for URL query parameters.
#[allow(dead_code)] // Called from run_oauth_flow.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}
