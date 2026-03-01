//! Slack integration for opman.
//!
//! Provides self-chat DM messaging via Slack Socket Mode (WebSocket) with:
//! - OAuth 2.0 authentication flow (temporary local HTTP server for callback)
//! - AI-powered project detection (triage session)
//! - Session routing to free sessions in detected project
//! - Response batching and relay back to Slack threads
//! - Thread reply handling via system message injection
//!
//! Credentials stored in `~/.config/opman/slack_auth.yaml` (separate from config.toml).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

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

// ── Socket Mode Client ─────────────────────────────────────────────────

// Note: The `SlackEvent` enum was an earlier design superseded by `SlackBackgroundEvent`.
// Removed to eliminate dead code.

/// Events sent from the Slack subsystem to the main app event loop.
/// Events sent from the Slack subsystem to the main app event loop.
#[derive(Debug)]
#[allow(dead_code)] // Variants and fields used at runtime via background event channel.
pub enum SlackBackgroundEvent {
    /// A new top-level message arrived; needs AI triage for project detection.
    IncomingMessage {
        text: String,
        channel: String,
        ts: String,
        user: String,
    },
    /// A thread reply arrived; route to existing session.
    IncomingThreadReply {
        text: String,
        channel: String,
        ts: String,
        thread_ts: String,
        user: String,
    },
    /// AI triage completed: identified target project and optional model.
    TriageResult {
        thread_ts: String,
        channel: String,
        original_text: String,
        /// The query rewritten by triage AI for the target project session.
        rewritten_query: Option<String>,
        project_path: Option<String>,
        model: Option<String>,
        error: Option<String>,
    },
    /// Response batch ready to send to Slack.
    ResponseBatch {
        channel: String,
        thread_ts: String,
        text: String,
    },
    /// OAuth flow completed.
    OAuthComplete(Result<SlackAuth>),
    /// Socket Mode connection status changed.
    ConnectionStatus(SlackConnectionStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    AuthError(String),
}

/// State maintained by the Slack subsystem.
#[allow(dead_code)]
pub struct SlackState {
    /// Current connection status.
    pub status: SlackConnectionStatus,
    /// Mapping from Slack thread_ts → (project_idx, session_id).
    /// Tracks which opman session each Slack thread is routed to.
    pub thread_sessions: HashMap<String, (usize, String)>,
    /// Pending response batches: session_id → accumulated text.
    pub response_buffers: HashMap<String, String>,
    /// Mapping from session_id → (channel, thread_ts) for response relay.
    pub session_threads: HashMap<String, (String, String)>,
    /// Sessions for which we have already relayed the assistant response.
    /// Prevents duplicate posts when SseSessionIdle fires multiple times.
    pub relayed_sessions: HashSet<String>,
    /// Number of messages in a session at the time Slack routed a message to it.
    /// Used to only relay messages that came after the routing point.
    pub session_msg_offset: HashMap<String, usize>,
    /// Abort handles for live relay watchers (session_id → AbortHandle).
    /// Used to cancel watchers when sessions are removed or app shuts down.
    pub relay_abort_handles: HashMap<String, tokio::task::AbortHandle>,
    /// Active streaming message timestamps: session_id → stream message ts.
    /// Used by the relay watcher to append content to an active stream.
    pub streaming_messages: HashMap<String, String>,
    /// Slack event log for debugging.
    pub event_log: Vec<SlackLogEntry>,
    /// Metrics counters.
    pub metrics: SlackMetrics,
}

impl SlackState {
    pub fn new() -> Self {
        Self {
            status: SlackConnectionStatus::Disconnected,
            thread_sessions: HashMap::new(),
            response_buffers: HashMap::new(),
            session_threads: HashMap::new(),
            relayed_sessions: HashSet::new(),
            session_msg_offset: HashMap::new(),
            relay_abort_handles: HashMap::new(),
            streaming_messages: HashMap::new(),
            event_log: Vec::new(),
            metrics: SlackMetrics::default(),
        }
    }

    #[allow(dead_code)]
    pub fn log(&mut self, level: SlackLogLevel, message: String) {
        self.event_log.push(SlackLogEntry {
            timestamp: Instant::now(),
            level,
            message,
        });
        // Keep only the last 200 entries.
        if self.event_log.len() > 200 {
            self.event_log.drain(0..self.event_log.len() - 200);
        }
    }
}

/// A single log entry in the Slack event log.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SlackLogEntry {
    pub timestamp: Instant,
    pub level: SlackLogLevel,
    pub message: String,
}

/// Severity level for Slack log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SlackLogLevel {
    Info,
    Warn,
    Error,
}

/// Metrics tracked by the Slack subsystem.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SlackMetrics {
    /// Number of messages routed to sessions.
    pub messages_routed: u64,
    /// Number of messages where triage failed.
    pub triage_failures: u64,
    /// Number of thread replies injected.
    pub thread_replies: u64,
    /// Number of response batches sent to Slack.
    pub batches_sent: u64,
    /// Number of reconnections.
    pub reconnections: u64,
    /// Timestamp of last successful message route.
    pub last_routed_at: Option<Instant>,
}

// ── Socket Mode WebSocket Connection ────────────────────────────────────

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Request a WebSocket URL from Slack's apps.connections.open endpoint.
async fn get_ws_url(client: &reqwest::Client, app_token: &str) -> Result<String> {
    let resp = client
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await
        .context("Failed to call apps.connections.open")?;

    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("apps.connections.open failed: {}", err);
    }

    body.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'url' in connections.open response")
}

/// Connect to Slack Socket Mode and process events.
/// Sends parsed events to the main loop via `event_tx`.
/// Reconnects automatically on disconnection.
pub async fn spawn_socket_mode(
    auth: SlackAuth,
    event_tx: mpsc::UnboundedSender<SlackBackgroundEvent>,
) {
    let client = reqwest::Client::new();
    let our_user_id = auth.user_id.clone();

    loop {
        // Notify connecting.
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Reconnecting,
        ));

        // Get a fresh WebSocket URL.
        let ws_url = match get_ws_url(&client, &auth.app_token).await {
            Ok(url) => url,
            Err(e) => {
                error!("Failed to get Socket Mode URL: {}", e);
                let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
                    SlackConnectionStatus::AuthError(e.to_string()),
                ));
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        debug!("Connecting to Socket Mode: {}", ws_url);

        let ws_result = tokio_tungstenite::connect_async(&ws_url).await;
        let ws_stream = match ws_result {
            Ok((stream, _)) => stream,
            Err(e) => {
                error!("WebSocket connection failed: {}", e);
                let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
                    SlackConnectionStatus::Disconnected,
                ));
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        info!("Socket Mode connected");
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Connected,
        ));

        let (mut ws_sink, mut ws_source) = ws_stream.split();

        // Process messages until disconnected.
        let disconnect_reason =
            process_socket_messages(&mut ws_sink, &mut ws_source, &event_tx, &our_user_id).await;

        warn!("Socket Mode disconnected: {}", disconnect_reason);
        let _ = event_tx.send(SlackBackgroundEvent::ConnectionStatus(
            SlackConnectionStatus::Disconnected,
        ));

        // Brief delay before reconnecting.
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Process incoming WebSocket messages from Socket Mode.
/// Returns a string describing why the connection ended.
async fn process_socket_messages(
    ws_sink: &mut SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
    ws_source: &mut SplitStream<WsStream>,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> String {
    use tokio_tungstenite::tungstenite::Message;

    while let Some(msg_result) = ws_source.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => return format!("WebSocket error: {}", e),
        };

        match msg {
            Message::Text(text) => {
                if let Err(e) = handle_socket_text(&text, ws_sink, event_tx, our_user_id).await {
                    warn!("Error handling Socket Mode message: {}", e);
                }
            }
            Message::Ping(data) => {
                if let Err(e) = ws_sink.send(Message::Pong(data)).await {
                    return format!("Failed to send pong: {}", e);
                }
            }
            Message::Close(_) => {
                return "Server closed connection".to_string();
            }
            _ => {}
        }
    }

    "Stream ended".to_string()
}

/// Handle a single Socket Mode text message (JSON envelope).
async fn handle_socket_text(
    text: &str,
    ws_sink: &mut SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> Result<()> {
    use tokio_tungstenite::tungstenite::Message;

    let envelope: serde_json::Value =
        serde_json::from_str(text).context("Invalid JSON from Socket Mode")?;

    let envelope_id = envelope
        .get("envelope_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let msg_type = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Always acknowledge the envelope to prevent retries.
    if !envelope_id.is_empty() {
        let ack = serde_json::json!({ "envelope_id": envelope_id });
        ws_sink
            .send(Message::Text(ack.to_string().into()))
            .await
            .context("Failed to send ack")?;
    }

    match msg_type {
        "events_api" => {
            if let Some(payload) = envelope.get("payload") {
                debug!("Socket Mode events_api payload: {}", payload);
                handle_events_api_payload(payload, event_tx, our_user_id)?;
            }
        }
        "disconnect" => {
            info!("Received disconnect from Slack (normal rotation)");
        }
        "hello" => {
            info!("Socket Mode hello received — connection ready");
        }
        other => {
            debug!("Unhandled Socket Mode envelope type: {}", other);
        }
    }

    Ok(())
}

/// Parse an Events API payload and emit the appropriate SlackBackgroundEvent.
fn handle_events_api_payload(
    payload: &serde_json::Value,
    event_tx: &mpsc::UnboundedSender<SlackBackgroundEvent>,
    our_user_id: &str,
) -> Result<()> {
    let event = payload
        .get("event")
        .context("Missing 'event' in events_api payload")?;

    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

    debug!(
        "Slack event: type={}, user={}, subtype={}, bot_id={}, channel_type={}",
        event_type,
        event
            .get("user")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("subtype")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("bot_id")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
        event
            .get("channel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("(none)"),
    );

    if event_type != "message" {
        debug!("Ignoring non-message event type: {}", event_type);
        return Ok(());
    }

    // Ignore bot-generated messages (our own replies, other bots).
    // Only filter the specific "bot_message" subtype — normal user messages
    // in DMs can carry other subtypes (e.g. "file_share") that we want to accept.
    if event.get("bot_id").is_some() {
        debug!("Ignoring bot message (bot_id present)");
        return Ok(());
    }
    let subtype = event.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
    if subtype == "bot_message" || subtype == "bot_add" || subtype == "bot_remove" {
        debug!("Ignoring bot subtype: {}", subtype);
        return Ok(());
    }

    let user = event.get("user").and_then(|v| v.as_str()).unwrap_or("");
    // Only process messages from ourselves (self-chat DM).
    if user != our_user_id {
        debug!(
            "Ignoring message from other user: {} (expected {})",
            user, our_user_id
        );
        return Ok(());
    }

    let text = event.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let channel = event.get("channel").and_then(|v| v.as_str()).unwrap_or("");
    let ts = event.get("ts").and_then(|v| v.as_str()).unwrap_or("");
    let thread_ts = event.get("thread_ts").and_then(|v| v.as_str());

    if text.is_empty() || channel.is_empty() || ts.is_empty() {
        debug!(
            "Ignoring message with empty fields: text={}, channel={}, ts={}",
            !text.is_empty(),
            !channel.is_empty(),
            !ts.is_empty()
        );
        return Ok(());
    }

    if let Some(parent_ts) = thread_ts {
        // This is a reply in a thread.
        info!(
            "Slack incoming thread reply: channel={} ts={} thread_ts={}",
            channel, ts, parent_ts
        );
        let _ = event_tx.send(SlackBackgroundEvent::IncomingThreadReply {
            text: text.to_string(),
            channel: channel.to_string(),
            ts: ts.to_string(),
            thread_ts: parent_ts.to_string(),
            user: user.to_string(),
        });
    } else {
        // Top-level message — needs triage.
        info!(
            "Slack incoming message: channel={} ts={} text={}...",
            channel,
            ts,
            &text[..text.len().min(50)]
        );
        let _ = event_tx.send(SlackBackgroundEvent::IncomingMessage {
            text: text.to_string(),
            channel: channel.to_string(),
            ts: ts.to_string(),
            user: user.to_string(),
        });
    }

    Ok(())
}

// ── Slack Web API Helpers ───────────────────────────────────────────────

/// Post a message to a Slack channel/thread.
pub async fn post_message(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    text: &str,
    thread_ts: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "channel": channel,
        "text": text,
    });
    if let Some(ts) = thread_ts {
        body["thread_ts"] = serde_json::Value::String(ts.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to post Slack message")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.postMessage failed: {}", err);
    }

    result
        .get("ts")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'ts' in postMessage response")
}

/// Update (edit) an existing Slack message.
#[allow(dead_code)]
pub async fn update_message(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
    text: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "channel": channel,
        "ts": ts,
        "text": text,
    });

    let resp = client
        .post("https://slack.com/api/chat.update")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to update Slack message")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.update failed: {}", err);
    }

    Ok(())
}

// ── Streaming API ───────────────────────────────────────────────────────

/// Start a new streaming message in a Slack thread.
/// Returns the stream message `ts` on success.
///
/// `task_display_mode` can be `"timeline"` (default, sequential task cards)
/// or `"plan"` (grouped task display).
pub async fn start_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    thread_ts: &str,
    markdown_text: Option<&str>,
    chunks: Option<&[serde_json::Value]>,
    task_display_mode: Option<&str>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "channel": channel,
        "thread_ts": thread_ts,
    });

    // Slack does not allow both top-level `markdown_text` AND `chunks` in the
    // same request.  When we have chunks, embed the text as a leading
    // `markdown_text` chunk inside the chunks array instead.
    match (markdown_text, chunks) {
        (Some(text), Some(c)) => {
            let mut all_chunks: Vec<serde_json::Value> = Vec::with_capacity(c.len() + 1);
            all_chunks.push(serde_json::json!({
                "type": "markdown_text",
                "text": text,
            }));
            all_chunks.extend(c.iter().cloned());
            body["chunks"] = serde_json::Value::Array(all_chunks);
        }
        (Some(text), None) => {
            body["markdown_text"] = serde_json::Value::String(text.to_string());
        }
        (None, Some(c)) => {
            body["chunks"] = serde_json::Value::Array(c.to_vec());
        }
        (None, None) => {}
    }
    if let Some(mode) = task_display_mode {
        body["task_display_mode"] = serde_json::Value::String(mode.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.startStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to start Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.startStream failed: {}", err);
    }

    result
        .get("ts")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .context("Missing 'ts' in startStream response")
}

/// Append content to an active streaming message.
pub async fn append_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
    markdown_text: &str,
    chunks: Option<&[serde_json::Value]>,
) -> Result<()> {
    let mut body = serde_json::json!({
        "channel": channel,
        "ts": ts,
    });

    // Slack does not allow both top-level `markdown_text` AND `chunks`.
    // When chunks are present, embed the text as a `markdown_text` chunk.
    if let Some(c) = chunks {
        let mut all_chunks: Vec<serde_json::Value> = Vec::with_capacity(c.len() + 1);
        if !markdown_text.is_empty() {
            all_chunks.push(serde_json::json!({
                "type": "markdown_text",
                "text": markdown_text,
            }));
        }
        all_chunks.extend(c.iter().cloned());
        body["chunks"] = serde_json::Value::Array(all_chunks);
    } else {
        body["markdown_text"] = serde_json::Value::String(markdown_text.to_string());
    }

    let resp = client
        .post("https://slack.com/api/chat.appendStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to append to Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.appendStream failed: {}", err);
    }

    Ok(())
}

/// Stop (finalize) an active streaming message.
pub async fn stop_stream(
    client: &reqwest::Client,
    bot_token: &str,
    channel: &str,
    ts: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "channel": channel,
        "ts": ts,
    });

    let resp = client
        .post("https://slack.com/api/chat.stopStream")
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .context("Failed to stop Slack stream")?;

    let result: serde_json::Value = resp.json().await?;
    if result.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = result
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown");
        anyhow::bail!("chat.stopStream failed: {}", err);
    }

    Ok(())
}

/// Split text into chunks that fit within Slack's 40,000 character limit.
/// Tries to split on newline boundaries for cleanliness.
pub fn chunk_for_slack(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to find a newline near the limit to split cleanly.
        let search_start = max_chars.saturating_sub(200);
        let split_at = remaining[search_start..max_chars]
            .rfind('\n')
            .map(|i| search_start + i + 1)
            .unwrap_or(max_chars);

        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }

    chunks
}

/// Convert standard Markdown to Slack mrkdwn format.
///
/// Key differences handled:
/// - `**bold**` → `*bold*`
/// - `*italic*` (when not inside bold) → `_italic_`
/// - `~~strike~~` → `~strike~`
/// - `[text](url)` → `<url|text>`
/// - `# heading` → `*heading*` (bold, Slack has no headings)
/// - Markdown tables → fenced code blocks (Slack has no table syntax)
/// - Fenced code blocks and inline code pass through unchanged.
pub fn markdown_to_slack_mrkdwn(md: &str) -> String {
    // Pre-pass: convert markdown tables to code blocks.
    let preprocessed = convert_markdown_tables(md);
    convert_inline_markdown(&preprocessed)
}

/// Detect markdown tables and wrap them in fenced code blocks so they render
/// as monospace in Slack (which has no native table support).
///
/// A markdown table is identified as a sequence of lines where:
/// - Each line contains at least one `|` character
/// - One of the early lines is a separator row (e.g. `|---|---|`)
fn convert_markdown_tables(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut result = String::with_capacity(md.len() + 128);
    let mut i = 0;
    let total = lines.len();
    // Track whether the original text ended with a newline.
    let ends_with_newline = md.ends_with('\n');

    while i < total {
        let line = lines[i];

        // Check if this line looks like a table row (contains `|`).
        // Also require the next line to be a separator row for a valid table.
        if line.contains('|') && i + 1 < total && is_table_separator(lines[i + 1]) {
            // Found a table. Collect all contiguous table rows.
            let table_start = i;
            let mut table_end = i;
            while table_end < total && is_table_line(lines[table_end]) {
                table_end += 1;
            }

            // Check if already inside a code fence (crude check: look back for
            // unmatched ```). We skip conversion if so.
            let preceding = &result;
            let fence_count = preceding.matches("```").count();
            if fence_count % 2 == 1 {
                // Inside a code block — pass through as-is.
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
            } else {
                result.push_str("```\n");
                for j in table_start..table_end {
                    result.push_str(lines[j]);
                    result.push('\n');
                }
                result.push_str("```\n");
            }
            i = table_end;
        } else {
            result.push_str(line);
            if i + 1 < total || ends_with_newline {
                result.push('\n');
            }
            i += 1;
        }
    }

    result
}

/// Check if a line is a markdown table separator row (e.g. `| --- | --- |` or `|:---:|---:|`).
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    // After removing `|`, `:`, `-`, and whitespace, nothing should remain.
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '|' | '-' | ':' | ' '))
        .collect();
    cleaned.is_empty() && trimmed.contains('-')
}

/// Check if a line looks like part of a markdown table (contains `|`).
fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    // A table line should contain `|`. Empty lines break the table.
    !trimmed.is_empty() && trimmed.contains('|')
}

/// The character-level inline markdown to Slack mrkdwn converter.
fn convert_inline_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut chars = md.chars().peekable();
    // Track whether we are inside a fenced code block (```).
    let mut in_fenced_block = false;
    // Track whether we are inside an inline code span (`).
    let mut in_inline_code = false;
    // Track whether we are at the start of a line (for heading detection).
    let mut at_line_start = true;

    while let Some(c) = chars.next() {
        // ── Fenced code block toggle ───────────────────────────────
        if c == '`' && chars.peek() == Some(&'`') {
            // Check for triple backtick.
            let second = chars.next(); // consume 2nd `
            if chars.peek() == Some(&'`') {
                let third = chars.next(); // consume 3rd `
                in_fenced_block = !in_fenced_block;
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                if let Some(ch) = third {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            } else {
                // Only two backticks — not a fence, push them through.
                result.push(c);
                if let Some(ch) = second {
                    result.push(ch);
                }
                at_line_start = false;
                continue;
            }
        }

        // Inside fenced code blocks, pass everything through verbatim.
        if in_fenced_block {
            if c == '\n' {
                at_line_start = true;
            } else {
                at_line_start = false;
            }
            result.push(c);
            continue;
        }

        // ── Inline code toggle ─────────────────────────────────────
        if c == '`' {
            in_inline_code = !in_inline_code;
            result.push(c);
            at_line_start = false;
            continue;
        }

        // Inside inline code, pass through verbatim.
        if in_inline_code {
            result.push(c);
            at_line_start = false;
            continue;
        }

        // ── Headings at line start ─────────────────────────────────
        if c == '#' && at_line_start {
            // Consume all leading '#' and optional space.
            while chars.peek() == Some(&'#') {
                chars.next();
            }
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            // Collect the heading text until end of line.
            let mut heading = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '\n' {
                    break;
                }
                heading.push(chars.next().unwrap());
            }
            // Render as bold in Slack.
            result.push('*');
            result.push_str(heading.trim());
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Bold: **text** → *text* ────────────────────────────────
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            result.push('*');
            at_line_start = false;
            continue;
        }

        // ── Italic: standalone *text* → _text_ ────────────────────
        // At this point a single `*` that wasn't part of `**` is italic.
        if c == '*' {
            result.push('_');
            at_line_start = false;
            continue;
        }

        // ── Strikethrough: ~~text~~ → ~text~ ───────────────────────
        if c == '~' && chars.peek() == Some(&'~') {
            chars.next(); // consume second ~
            result.push('~');
            at_line_start = false;
            continue;
        }

        // ── Links: [text](url) → <url|text> ───────────────────────
        if c == '[' {
            // Try to parse a markdown link.
            let mut link_text = String::new();
            let mut found_link = false;
            let mut inner_chars = chars.clone();
            let mut bracket_depth = 1;

            // Collect text inside brackets.
            while let Some(ch) = inner_chars.next() {
                if ch == '[' {
                    bracket_depth += 1;
                } else if ch == ']' {
                    bracket_depth -= 1;
                    if bracket_depth == 0 {
                        // Check if followed by (url)
                        if inner_chars.peek() == Some(&'(') {
                            inner_chars.next(); // consume '('
                            let mut url = String::new();
                            let mut paren_depth = 1;
                            while let Some(uc) = inner_chars.next() {
                                if uc == '(' {
                                    paren_depth += 1;
                                    url.push(uc);
                                } else if uc == ')' {
                                    paren_depth -= 1;
                                    if paren_depth == 0 {
                                        found_link = true;
                                        break;
                                    }
                                    url.push(uc);
                                } else {
                                    url.push(uc);
                                }
                            }
                            if found_link {
                                result.push('<');
                                result.push_str(&url);
                                result.push('|');
                                result.push_str(&link_text);
                                result.push('>');
                                chars = inner_chars;
                            }
                        }
                        break;
                    }
                }
                if bracket_depth > 0 {
                    link_text.push(ch);
                }
            }
            if !found_link {
                result.push(c);
            }
            at_line_start = false;
            continue;
        }

        // ── Newline tracking ───────────────────────────────────────
        if c == '\n' {
            at_line_start = true;
            result.push(c);
            continue;
        }

        at_line_start = false;
        result.push(c);
    }

    result
}

// ── Triage: AI Project Detection ────────────────────────────────────────

/// The triage project directory: `~/.config/opman/slack/`.
/// This is a synthetic project used solely for the AI triage session.
///
/// Uses the same primary / XDG-fallback lookup as [`SlackAuth::auth_path`],
/// following symlinks transparently.
pub fn triage_project_dir() -> Result<PathBuf> {
    let subpath = std::path::Path::new("opman").join("slack");

    // 1. Primary: dirs::config_dir()/opman/slack/
    if let Some(primary) = dirs::config_dir() {
        let p = primary.join(&subpath);
        if std::fs::metadata(&p).is_ok() {
            return Ok(p);
        }
    }

    // 2. Fallback: ~/.config/opman/slack/
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join(&subpath);
        if std::fs::metadata(&p).is_ok() {
            return Ok(p);
        }
    }

    // 3. Neither exists – return the primary path (callers will create it).
    let dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("opman")
        .join("slack");
    Ok(dir)
}

/// Build the system prompt for the triage AI session.
/// Includes the list of known projects so the AI can match.
pub fn build_triage_prompt(projects: &[(String, String)], user_text: &str) -> String {
    let project_list: String = projects
        .iter()
        .enumerate()
        .map(|(i, (name, path))| format!("  {}. \"{}\" — {}", i + 1, name, path))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are a project router for a software development environment manager called opman.

The user sent a message via Slack. Your job is to:
1. Determine which project they are referring to
2. Whether they specified a model preference (e.g., "use claude", "with gpt-4", "sonnet")
3. Rewrite their message to contain ONLY the actual task or question — strip ALL routing metadata

Available projects:
{project_list}

User's message:
"{user_text}"

Respond with EXACTLY this JSON format (no markdown, no explanation):
{{"project_name": "<name>", "project_path": "<path>", "model": "<model or null>", "rewritten_query": "<the user's actual task/question with ALL routing metadata removed>", "confidence": <0.0-1.0>}}

If you cannot determine the project with reasonable confidence (>0.5), respond:
{{"project_name": null, "project_path": null, "model": null, "rewritten_query": null, "confidence": 0.0, "error": "Could not determine which project you mean. Please specify the project name."}}

Rules:
- Match project names loosely (abbreviations, partial names are OK).
- If the message mentions a file path, match the project whose path is a prefix.
- If only one project exists, assume it's the target unless the message explicitly says otherwise.
- For model detection, look for keywords like "claude", "sonnet", "opus", "gpt", "o1", "gemini", etc.
- CRITICAL — rewritten_query rules:
  - Remove ALL project names, project paths, session names, session IDs, model preferences, and routing instructions.
  - Strip phrases like "in the X project", "send this to Y", "direct this to Z", "using model W", "in session ABC", "@session", "@list-sessions", etc.
  - The rewritten_query must read as a clean, standalone message — as if the user typed it directly to a coding assistant with no routing context.
  - Keep ONLY the substantive task, question, or instruction.
  - If the entire message is just routing with no real task, set rewritten_query to the original message.
"#
    )
}

/// Parse the triage AI response to extract project path, model, rewritten query, and error.
/// Returns (project_path, model, rewritten_query, error).
pub fn parse_triage_response(
    response_text: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    // Try to parse as JSON first.
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(response_text) {
        let project_path = val
            .get("project_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let model = val
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .map(|s| s.to_string());
        let rewritten_query = val
            .get("rewritten_query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .map(|s| s.to_string());
        let error = val
            .get("error")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        return (project_path, model, rewritten_query, error);
    }

    // If JSON parsing fails, try to extract from the raw text.
    warn!("Triage response was not valid JSON, attempting text extraction");
    (
        None,
        None,
        None,
        Some("Could not parse triage response. Please specify the project explicitly.".to_string()),
    )
}

// ── Top-Level @ Command Helpers ─────────────────────────────────────────

/// Convert an epoch-seconds timestamp to a human-readable relative time string
/// like "2 minutes ago", "3 hours ago", "yesterday", etc.
fn human_readable_elapsed(epoch_secs: u64) -> String {
    let now = chrono::Utc::now().timestamp() as u64;
    if epoch_secs == 0 || epoch_secs > now {
        return "unknown".to_string();
    }
    let diff = now - epoch_secs;
    if diff < 60 {
        return "just now".to_string();
    }
    let minutes = diff / 60;
    if minutes < 60 {
        return if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", minutes)
        };
    }
    let hours = minutes / 60;
    if hours < 24 {
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        };
    }
    let days = hours / 24;
    if days == 1 {
        return "yesterday".to_string();
    }
    if days < 30 {
        return format!("{} days ago", days);
    }
    let months = days / 30;
    if months < 12 {
        return if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        };
    }
    let years = months / 12;
    if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{} years ago", years)
    }
}

/// Handle the `@list-projects` top-level command.
/// Posts a list of all configured projects (excluding slack-triage) to the Slack
/// channel as a threaded reply.
pub async fn handle_list_projects_command(
    projects: &[(String, String)], // (name, path)
    channel: &str,
    ts: &str,
    bot_token: &str,
) {
    let client = reqwest::Client::new();
    let project_list = if projects.is_empty() {
        "No projects configured.".to_string()
    } else {
        projects
            .iter()
            .map(|(name, path)| format!("• *{}*  `{}`", name, path))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let msg = format!(
        ":package: *Available Projects ({})* :\n{}",
        projects.len(),
        project_list,
    );
    let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
}

/// Metadata about a session, passed to @ command handlers.
#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub parent_id: String,
    pub updated: u64,
    pub project_idx: usize,
    pub project_name: String,
    pub project_dir: String,
}

/// Build a specialized AI prompt that asks the triage AI to match a user query
/// against the list of known projects and return the matching project path.
fn build_project_match_prompt(projects: &[(String, String)], user_query: &str) -> String {
    let project_list: String = projects
        .iter()
        .enumerate()
        .map(|(i, (name, path))| format!("  {}. \"{}\" — {}", i + 1, name, path))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are helping route a Slack command that lists sessions in a project.

The user typed `@list-sessions {user_query}`.
Your job is to determine which project they are referring to.

Available projects:
{project_list}

Respond with EXACTLY this JSON format (no markdown, no explanation):
{{"project_name": "<name>", "project_path": "<path>", "confidence": <0.0-1.0>}}

If you cannot determine the project with reasonable confidence (>0.5), respond:
{{"project_name": null, "project_path": null, "confidence": 0.0, "error": "Could not determine which project you mean. Available: {available}"}}

Rules:
- Match project names loosely (abbreviations, partial names, typos are OK).
- If only one project exists, assume it is the target.
- The "error" field should list the available project names so the user knows what to type.
"#,
        user_query = user_query,
        project_list = project_list,
        available = projects
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Build a specialized AI prompt that asks the triage AI to match a user query
/// against the list of all sessions and return the matching session ID.
fn build_session_match_prompt(sessions: &[SessionMeta], user_query: &str) -> String {
    let session_list: String = sessions
        .iter()
        .filter(|s| s.parent_id.is_empty()) // skip subagents
        .map(|s| {
            let short_id = &s.id[..8.min(s.id.len())];
            let title = if s.title.is_empty() {
                "(untitled)".to_string()
            } else {
                s.title.clone()
            };
            let updated = if s.updated > 0 {
                chrono::DateTime::from_timestamp(s.updated as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "unknown".to_string()
            };
            format!(
                "  - ID: {} | Title: \"{}\" | Project: \"{}\" | Updated: {}",
                short_id, title, s.project_name, updated
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are helping route a Slack command to a specific coding session.

The user typed `@session {user_query} <message>`.
Your job is to:
1. Determine which session they are referring to by "{user_query}"
2. Rewrite the message portion to contain ONLY the actual task or question — strip ALL routing metadata

Available sessions:
{session_list}

Respond with EXACTLY this JSON format (no markdown, no explanation):
{{"session_id": "<full session id>", "session_title": "<title>", "project_name": "<project>", "rewritten_query": "<the user's actual task/question with ALL routing metadata removed>", "confidence": <0.0-1.0>}}

If you cannot determine the session with reasonable confidence (>0.5), respond:
{{"session_id": null, "confidence": 0.0, "error": "Could not determine which session you mean.", "candidates": ["<id_prefix>: <title> (project)", ...]}}

Rules:
- Match session titles loosely (abbreviations, partial names, keywords are OK).
- Also match by session ID prefix (e.g. "abc123" should match a session whose ID starts with "abc123").
- Prefer more recently updated sessions if multiple match equally well.
- The "candidates" field in error responses should list the top 5 closest matches.
- CRITICAL — rewritten_query rules:
  - The message the user wants to send follows the session identifier. Extract it and clean it.
  - Remove ALL session names, session IDs, project names, project paths, and routing instructions from the message.
  - Strip phrases like "in session X", "to session Y", "in the Z project", "@session", etc.
  - The rewritten_query must read as a clean, standalone message — as if the user typed it directly to a coding assistant.
  - Keep ONLY the substantive task, question, or instruction.
  - If there is no message beyond the session identifier, set rewritten_query to null.
"#,
        user_query = user_query,
        session_list = session_list,
    )
}
/// Helper: get the triage session ID by fetching sessions from the triage project.
/// Returns the session ID, or an empty string if none found.
async fn get_triage_session_id(
    client: &reqwest::Client,
    base_url: &str,
    triage_dir: &str,
) -> String {
    let sessions_url = format!("{}/session", base_url);
    let sessions_resp = client
        .get(&sessions_url)
        .header("x-opencode-directory", triage_dir)
        .header("Accept", "application/json")
        .send()
        .await;

    match sessions_resp {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
                arr.iter().collect()
            } else if let Some(obj) = body.as_object() {
                obj.values().collect()
            } else {
                vec![]
            };
            let triage_session = items.iter().find(|s| {
                let dir = s.get("directory").and_then(|v| v.as_str()).unwrap_or("");
                dir == triage_dir
            });
            triage_session
                .and_then(|s| s.get("id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string()
        }
        Err(e) => {
            warn!("@ command: failed to fetch triage sessions: {}", e);
            String::new()
        }
    }
}

/// Helper: send a prompt to the triage AI and wait for its JSON response.
/// Returns the raw AI response text, or an error string.
async fn send_triage_and_wait(
    client: &reqwest::Client,
    base_url: &str,
    triage_dir: &str,
    session_id: &str,
    prompt: &str,
) -> Result<String, String> {
    send_user_message(client, base_url, triage_dir, session_id, prompt)
        .await
        .map_err(|e| format!("Failed to send triage prompt: {}", e))?;

    // Wait for the AI to respond.
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let messages = fetch_all_session_messages(client, base_url, triage_dir, session_id)
        .await
        .map_err(|e| format!("Failed to fetch triage response: {}", e))?;

    let ai_response = messages
        .iter()
        .rev()
        .find(|(role, _)| role == "assistant")
        .map(|(_, text)| text.clone())
        .unwrap_or_default();

    if ai_response.is_empty() {
        Err("Triage AI did not respond.".to_string())
    } else {
        Ok(ai_response)
    }
}

/// Handle `@list-sessions <fuzzy project name>` using AI triage for project matching.
///
/// Sends a specialized prompt to the triage AI to fuzzy-match the project,
/// then fetches and formats the last 5 sessions for that project.
pub async fn handle_list_sessions_command(
    query: &str,
    projects: &[(String, String)], // (name, path)
    sessions: &[SessionMeta],      // all sessions across projects
    channel: &str,
    ts: &str,
    bot_token: &str,
    base_url: &str,
) {
    let client = reqwest::Client::new();

    // Get the triage project directory.
    let triage_dir = match triage_project_dir() {
        Ok(d) => d.to_string_lossy().to_string(),
        Err(e) => {
            let msg = format!(
                ":x: Internal error — could not locate triage project: {}",
                e
            );
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    // Get triage session.
    let session_id = get_triage_session_id(&client, base_url, &triage_dir).await;
    if session_id.is_empty() {
        let msg = ":x: No triage session available. Please ensure the Slack triage project has at least one session.";
        let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // Build specialized project-matching prompt and send to triage AI.
    let prompt = build_project_match_prompt(projects, query);
    let ai_response =
        match send_triage_and_wait(&client, base_url, &triage_dir, &session_id, &prompt).await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!(":x: Triage failed: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
                return;
            }
        };

    // Parse the AI response as JSON.
    let parsed: serde_json::Value = match serde_json::from_str(&ai_response) {
        Ok(v) => v,
        Err(_) => {
            // Try to extract JSON from markdown code fences.
            let stripped = ai_response
                .trim()
                .strip_prefix("```json")
                .or_else(|| ai_response.trim().strip_prefix("```"))
                .unwrap_or(&ai_response)
                .trim()
                .strip_suffix("```")
                .unwrap_or(&ai_response)
                .trim();
            match serde_json::from_str(stripped) {
                Ok(v) => v,
                Err(_) => {
                    let available = projects
                        .iter()
                        .map(|(n, _)| format!("`{}`", n))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let msg = format!(
                        ":warning: Could not parse AI response for project matching. Available projects: {}",
                        available
                    );
                    let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
                    return;
                }
            }
        }
    };

    // Check for error.
    if let Some(err) = parsed
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let msg = format!(":warning: {}", err);
        let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    let project_path = parsed
        .get("project_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let project_name = parsed
        .get("project_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    if project_path.is_empty() {
        let available = projects
            .iter()
            .map(|(n, _)| format!("`{}`", n))
            .collect::<Vec<_>>()
            .join(", ");
        let msg = format!(
            ":warning: No project matched \"{}\". Available: {}",
            query, available
        );
        let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    // Filter sessions for this project (skip subagents).
    let mut project_sessions: Vec<&SessionMeta> = sessions
        .iter()
        .filter(|s| s.project_dir == project_path && s.parent_id.is_empty())
        .collect();

    // Sort by updated time descending, take last 5.
    project_sessions.sort_by(|a, b| b.updated.cmp(&a.updated));
    let recent: Vec<_> = project_sessions.into_iter().take(5).collect();

    if recent.is_empty() {
        let msg = format!(
            ":inbox_tray: No sessions found in project *{}* (`{}`)",
            project_name, project_path
        );
        let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    let session_list = recent
        .iter()
        .map(|s| {
            let short_id = &s.id[..8.min(s.id.len())];
            let title = if s.title.is_empty() {
                format!("Session {}", short_id)
            } else {
                s.title.clone()
            };
            let updated = if s.updated > 0 {
                human_readable_elapsed(s.updated)
            } else {
                "unknown".to_string()
            };
            format!("• *{}*  (ID: `{}`, updated: {})", title, short_id, updated)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let msg = format!(
        ":clipboard: *Recent sessions in {} ({})* :\n{}",
        project_name,
        recent.len(),
        session_list,
    );
    let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
}

/// Handle `@session <fuzzy session name> <message>` using AI triage for session matching.
///
/// Sends a specialized prompt to the triage AI to fuzzy-match the session,
/// then routes the message to the matched session.
pub async fn handle_session_command(
    session_query: &str,
    message_text: &str,
    all_sessions: &[SessionMeta],
    channel: &str,
    ts: &str,
    bot_token: &str,
    base_url: &str,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    // Get the triage project directory.
    let triage_dir = match triage_project_dir() {
        Ok(d) => d.to_string_lossy().to_string(),
        Err(e) => {
            let msg = format!(
                ":x: Internal error — could not locate triage project: {}",
                e
            );
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    // Get triage session.
    let triage_session_id = get_triage_session_id(&client, base_url, &triage_dir).await;
    if triage_session_id.is_empty() {
        let msg = ":x: No triage session available. Please ensure the Slack triage project has at least one session.";
        let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // Build specialized session-matching prompt and send to triage AI.
    let prompt = build_session_match_prompt(all_sessions, session_query);
    let ai_response =
        match send_triage_and_wait(&client, base_url, &triage_dir, &triage_session_id, &prompt)
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!(":x: Triage failed: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
                return;
            }
        };

    // Parse the AI response as JSON.
    let parsed: serde_json::Value = match serde_json::from_str(&ai_response) {
        Ok(v) => v,
        Err(_) => {
            // Try to extract JSON from markdown code fences.
            let stripped = ai_response
                .trim()
                .strip_prefix("```json")
                .or_else(|| ai_response.trim().strip_prefix("```"))
                .unwrap_or(&ai_response)
                .trim()
                .strip_suffix("```")
                .unwrap_or(&ai_response)
                .trim();
            match serde_json::from_str(stripped) {
                Ok(v) => v,
                Err(_) => {
                    let msg = ":warning: Could not parse AI response for session matching. Use `@list-sessions <project>` to find session names.";
                    let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
                    return;
                }
            }
        }
    };

    // Check for error / low confidence.
    if let Some(err) = parsed
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let mut msg = format!(":warning: {}", err);
        if let Some(candidates) = parsed.get("candidates").and_then(|v| v.as_array()) {
            let list: Vec<String> = candidates
                .iter()
                .filter_map(|c| c.as_str().map(|s| format!("• {}", s)))
                .collect();
            if !list.is_empty() {
                msg.push_str(&format!(
                    "\n\nDid you mean one of these?\n{}",
                    list.join("\n")
                ));
            }
        }
        let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    // Extract matched session ID. The AI returns the full session ID.
    let matched_session_id_raw = parsed
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    if matched_session_id_raw.is_empty() {
        let msg = ":warning: AI could not match a session. Use `@list-sessions <project>` to find session names.";
        let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // The AI might return a prefix; find the full session by prefix match.
    let matched_meta = all_sessions
        .iter()
        .find(|s| s.id == matched_session_id_raw || s.id.starts_with(matched_session_id_raw));

    let matched_meta = match matched_meta {
        Some(m) => m,
        None => {
            let msg = format!(
                ":warning: AI matched session ID `{}` but it was not found in the session list.",
                matched_session_id_raw
            );
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    let session_id = matched_meta.id.clone();
    let session_name = if matched_meta.title.is_empty() {
        format!("Session {}", &session_id[..8.min(session_id.len())])
    } else {
        matched_meta.title.clone()
    };
    let project_name = matched_meta.project_name.clone();
    let project_dir = matched_meta.project_dir.clone();
    let pidx = matched_meta.project_idx;

    // Record thread→session mapping.
    {
        let mut s = slack_state.lock().await;
        s.thread_sessions
            .insert(ts.to_string(), (pidx, session_id.clone()));
        s.session_threads
            .insert(session_id.clone(), (channel.to_string(), ts.to_string()));
        s.metrics.messages_routed += 1;
        s.metrics.last_routed_at = Some(std::time::Instant::now());
        s.log(
            SlackLogLevel::Info,
            format!(
                "@ command routed to project \"{}\" session {}",
                project_name,
                &session_id[..8.min(session_id.len())]
            ),
        );
    }

    // Record current message offset so relay only shows new messages.
    match fetch_all_session_messages(&client, base_url, &project_dir, &session_id).await {
        Ok(msgs) => {
            let mut s = slack_state.lock().await;
            s.session_msg_offset.insert(session_id.clone(), msgs.len());
            debug!(
                "@ command: recorded msg offset {} for session {}",
                msgs.len(),
                session_id
            );
        }
        Err(e) => {
            warn!(
                "@ command: failed to fetch msg offset for session {}: {}",
                session_id, e
            );
        }
    }

    // Use the AI's rewritten query if available, otherwise fall back to raw message.
    let final_message = parsed
        .get("rewritten_query")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "null")
        .unwrap_or(message_text);

    // Send the user message to the session.
    match send_user_message(&client, base_url, &project_dir, &session_id, final_message).await {
        Ok(()) => {
            info!("@ command: user message sent to session {}", session_id);
            let ack = format!(
                "relayed to project: {}, session: {}",
                project_name, session_name
            );
            let _ = post_message(&client, bot_token, channel, &ack, Some(ts)).await;
        }
        Err(e) => {
            error!("@ command: failed to send message to session: {}", e);
            let msg = format!(":x: Failed to send message: {}", e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    }

    // Spawn relay watcher (if not already running).
    let already_watching = {
        let s = slack_state.lock().await;
        s.relay_abort_handles.contains_key(&session_id)
    };
    if !already_watching {
        let handle = spawn_session_relay_watcher(
            session_id.clone(),
            project_dir,
            channel.to_string(),
            ts.to_string(),
            bot_token.to_string(),
            base_url.to_string(),
            buffer_secs,
            slack_state.clone(),
        );
        let mut s = slack_state.lock().await;
        s.relay_abort_handles
            .insert(session_id.clone(), handle.abort_handle());

        // Persist session map to disk.
        let map = SlackSessionMap {
            session_threads: s.session_threads.clone(),
            thread_sessions: s.thread_sessions.clone(),
            msg_offsets: s.session_msg_offset.clone(),
        };
        if let Err(e) = map.save() {
            warn!("@ command: failed to persist session map: {}", e);
        }
    }
}

// ── Response Batching ───────────────────────────────────────────────────

/// Spawn a background task that periodically checks for pending response batches
/// and sends them to Slack.
pub async fn spawn_response_batcher(
    auth: SlackAuth,
    state: Arc<Mutex<SlackState>>,
    batch_interval_secs: u64,
    _event_tx: mpsc::UnboundedSender<SlackBackgroundEvent>,
) {
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(Duration::from_secs(batch_interval_secs));

    loop {
        interval.tick().await;

        // Collect pending batches.
        let batches: Vec<(String, String, String)> = {
            let mut st = state.lock().await;
            let mut out = Vec::new();

            let session_ids: Vec<String> = st.response_buffers.keys().cloned().collect();
            for session_id in session_ids {
                if let Some(text) = st.response_buffers.remove(&session_id) {
                    if !text.is_empty() {
                        if let Some((channel, thread_ts)) = st.session_threads.get(&session_id) {
                            out.push((channel.clone(), thread_ts.clone(), text));
                        }
                    }
                }
            }

            out
        };

        // Send each batch to Slack.
        let batch_count = batches.len();
        for (channel, thread_ts, text) in batches {
            let chunks = chunk_for_slack(&text, 39_000); // Leave some margin
            for chunk in chunks {
                match post_message(&client, &auth.bot_token, &channel, &chunk, Some(&thread_ts))
                    .await
                {
                    Ok(_) => debug!("Relayed response batch to Slack thread {}", thread_ts),
                    Err(e) => error!("Failed to relay to Slack: {}", e),
                }
                // Small delay between chunks to avoid rate limiting.
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        // Update metrics for batches sent via the batcher.
        if batch_count > 0 {
            let mut st = state.lock().await;
            st.metrics.batches_sent += batch_count as u64;
            st.log(
                SlackLogLevel::Info,
                format!("Batcher sent {} response batch(es)", batch_count),
            );
        }
    }
}

/// Find a "free" session in the given project — one that is not currently
/// actively processing a request.  Returns the **most recently used** session
/// among idle candidates (preferring the one whose context is freshest).
///
/// A session is considered free if:
/// - It is NOT a subagent session (no parent_id)
/// - It is NOT in `active_sessions` (not currently busy)
///
/// The `idle_minutes` parameter is ignored — any non-busy session qualifies.
pub fn find_free_session(
    sessions: &[crate::app::SessionInfo],
    active_sessions: &std::collections::HashSet<String>,
    _idle_minutes: u64,
) -> Option<String> {
    let mut candidates: Vec<&crate::app::SessionInfo> = sessions
        .iter()
        .filter(|s| {
            // Skip subagent sessions (those with a parent).
            if !s.parent_id.is_empty() {
                debug!(
                    "  session {} — skipped (subagent)",
                    &s.id[..8.min(s.id.len())]
                );
                return false;
            }
            // Skip sessions that are currently active/busy.
            if active_sessions.contains(&s.id) {
                debug!(
                    "  session {} — skipped (active/busy)",
                    &s.id[..8.min(s.id.len())]
                );
                return false;
            }
            debug!(
                "  session {} — eligible (not busy, updated={})",
                &s.id[..8.min(s.id.len())],
                s.time.updated,
            );
            true
        })
        .collect();

    debug!(
        "find_free_session: {} candidate(s) out of {} total sessions",
        candidates.len(),
        sessions.len()
    );

    // Sort by updated time descending — most recently used (highest timestamp) first.
    // This prefers sessions with fresher context.
    candidates.sort_by(|a, b| b.time.updated.cmp(&a.time.updated));

    let result = candidates.first().map(|s| s.id.clone());
    if let Some(ref sid) = result {
        debug!(
            "find_free_session: selected {} (most recently used)",
            &sid[..8.min(sid.len())]
        );
    } else {
        debug!("find_free_session: no free session found");
    }
    result
}

// ── Tool Call Formatting Helpers ─────────────────────────────────────────

/// Format a V2 tool part (`type: "tool"`) into a compact text representation.
///
/// V2 structure:
/// ```json
/// { "type": "tool", "tool": "bash", "callID": "...",
///   "state": { "status": "completed", "input": {...}, "output": "...", "title": "..." } }
/// ```
fn format_tool_part_v2(p: &serde_json::Value) -> String {
    let tool_name = p.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
    let state = match p.get("state") {
        Some(s) => s,
        None => return format!("`{}`", tool_name),
    };
    let status = state
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    match status {
        "completed" => {
            let title = state.get("title").and_then(|t| t.as_str()).unwrap_or("");
            if title.is_empty() {
                format!("`{}` (done)", tool_name)
            } else {
                format!("`{}`: {}", tool_name, title)
            }
        }
        "running" => {
            let title = state.get("title").and_then(|t| t.as_str()).unwrap_or("");
            if title.is_empty() {
                format!("`{}` (running)", tool_name)
            } else {
                format!("`{}`: {} (running)", tool_name, title)
            }
        }
        "error" => {
            let err = state
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            // Truncate long error messages to keep Slack output readable.
            let mut end = 200.min(err.len());
            while !err.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            let short_err = &err[..end];
            format!("`{}` error: {}", tool_name, short_err)
        }
        "pending" => format!("`{}` (pending)", tool_name),
        _ => format!("`{}` ({})", tool_name, status),
    }
}

/// Format a V1 tool-invocation part (`type: "tool-invocation"`) into text.
///
/// V1 structure:
/// ```json
/// { "type": "tool-invocation",
///   "toolInvocation": { "state": "result", "toolName": "bash", "args": {...}, "result": "..." } }
/// ```
fn format_tool_part_v1(p: &serde_json::Value) -> String {
    let inv = match p.get("toolInvocation") {
        Some(i) => i,
        None => return "`tool` (unknown)".to_string(),
    };
    let tool_name = inv
        .get("toolName")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let state = inv
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    match state {
        "result" => format!("`{}` (done)", tool_name),
        "call" | "partial-call" => format!("`{}` (calling)", tool_name),
        _ => format!("`{}` ({})", tool_name, state),
    }
}

// ── Structured Tool Data ────────────────────────────────────────────────

/// A structured tool invocation extracted from an OpenCode session message.
/// Used to build Slack `task_update` streaming chunks.
#[derive(Debug, Clone)]
pub struct ToolPart {
    /// Tool name (e.g. "bash", "read", "edit").
    pub tool: String,
    /// Unique call ID for this invocation.
    pub call_id: String,
    /// Status: "pending", "running", "completed", "error".
    pub status: String,
    /// Human-readable title/summary of what the tool is doing.
    pub title: String,
    /// Optional output/result text (truncated for display).
    pub output: String,
}

impl ToolPart {
    /// Map OpenCode tool status to Slack `task_update` status values.
    /// Slack accepts: "pending", "in_progress", "complete", "error".
    pub fn slack_status(&self) -> &str {
        match self.status.as_str() {
            "completed" => "complete",
            "running" => "in_progress",
            "pending" => "pending",
            "error" => "error",
            // V1 states
            "result" => "complete",
            "call" | "partial-call" => "in_progress",
            _ => "in_progress",
        }
    }

    /// Convert this tool part into a Slack `task_update` chunk JSON value.
    pub fn to_task_chunk(&self) -> serde_json::Value {
        let mut chunk = serde_json::json!({
            "type": "task_update",
            "id": self.call_id,
            "title": if self.title.is_empty() {
                format!("`{}`", self.tool)
            } else {
                format!("`{}`: {}", self.tool, self.title)
            },
            "status": self.slack_status(),
        });
        if !self.output.is_empty() {
            // Strip XML-like tags and truncate output for Slack display.
            let cleaned = strip_xml_tags(&self.output);
            if !cleaned.is_empty() {
                // Use a larger limit for "task" (subagent) tool calls so that
                // the full subagent response is visible inside the task card.
                let max_len: usize = if self.tool == "task" { 4000 } else { 500 };
                let truncated = if cleaned.len() > max_len {
                    let mut end = max_len;
                    while !cleaned.is_char_boundary(end) && end > 0 {
                        end -= 1;
                    }
                    format!("{}\u{2026}", &cleaned[..end])
                } else {
                    cleaned
                };
                chunk["output"] = serde_json::Value::String(truncated.clone());
                // For error status, also populate `details` since Slack may
                // not render `output` for errored tasks in timeline view.
                if self.status == "error" {
                    chunk["details"] = serde_json::Value::String(truncated);
                }
            }
        }
        chunk
    }
}

/// Strip XML-like tags from tool output for cleaner Slack display.
/// Converts `<tag>content</tag>` to just `content` and removes self-closing tags.
fn strip_xml_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Clean up extra blank lines left behind by removed tags.
    let cleaned: Vec<&str> = result
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty())
        .collect();
    cleaned.join("\n")
}

/// Extract a `ToolPart` from a V2 tool part JSON value.
fn extract_tool_part_v2(p: &serde_json::Value) -> Option<ToolPart> {
    let tool = p.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
    let call_id = p
        .get("callID")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let state = p.get("state")?;
    let status = state
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown")
        .to_string();
    let title = state
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    // For error status, the message is in `state.error` rather than `state.output`.
    // Handle both string and object forms of the error field.
    let output = if status == "error" {
        let err_val = state.get("error");
        match err_val {
            Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
            Some(v) if v.is_object() => {
                // Try common sub-fields: "message", "text", then fall back to JSON.
                v.get("message")
                    .or_else(|| v.get("text"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string())
            }
            Some(v) => v.to_string(),
            None => {
                // Fallback: try `state.output` even for errors.
                state
                    .get("output")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string()
            }
        }
    } else {
        state
            .get("output")
            .and_then(|o| o.as_str())
            .unwrap_or("")
            .to_string()
    };

    Some(ToolPart {
        tool: tool.to_string(),
        call_id: if call_id.is_empty() {
            format!("tool_{}_{}", tool, status)
        } else {
            call_id
        },
        status,
        title,
        output,
    })
}

/// Extract a `ToolPart` from a V1 tool-invocation part JSON value.
fn extract_tool_part_v1(p: &serde_json::Value) -> Option<ToolPart> {
    let inv = p.get("toolInvocation")?;
    let tool = inv
        .get("toolName")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let state = inv
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let tool_call_id = inv.get("toolCallId").and_then(|c| c.as_str()).unwrap_or("");
    let result = inv.get("result").and_then(|r| r.as_str()).unwrap_or("");

    Some(ToolPart {
        tool: tool.to_string(),
        call_id: if tool_call_id.is_empty() {
            format!("v1_{}_{}", tool, state)
        } else {
            tool_call_id.to_string()
        },
        status: state.to_string(),
        title: String::new(),
        output: result.to_string(),
    })
}

/// A session message with both rendered text and structured tool data.
#[derive(Debug, Clone)]
pub struct StructuredMessage {
    pub role: String,
    /// Text content (from text parts, with tool parts formatted as text).
    pub text: String,
    /// Structured tool invocations found in this message.
    pub tools: Vec<ToolPart>,
}

/// Fetch messages for a session, returning structured data including tool parts.
/// This is the rich version of `fetch_all_session_messages` that preserves tool
/// metadata for use with Slack `task_update` streaming chunks.
pub async fn fetch_session_messages_with_tools(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
) -> Result<Vec<StructuredMessage>> {
    let url = format!("{}/session/{}/message", base_url, session_id);

    let response = client
        .get(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch session messages")?;

    let body: serde_json::Value = response.json().await?;

    let mut messages = Vec::new();
    let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.iter().collect()
    } else if let Some(obj) = body.as_object() {
        obj.values().collect()
    } else {
        vec![]
    };

    for item in items {
        let info = item.get("info");
        let role = info
            .and_then(|i| i.get("role"))
            .and_then(|r| r.as_str())
            .or_else(|| item.get("role").and_then(|r| r.as_str()))
            .unwrap_or("")
            .to_string();

        let mut text_parts = Vec::new();
        let mut tool_parts = Vec::new();

        if let Some(parts) = item.get("parts").and_then(|p| p.as_array()) {
            for p in parts {
                let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match ptype {
                    "text" | "" => {
                        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                            if !t.is_empty() {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                    "tool" => {
                        // Extract structured tool data for task_update chunks.
                        // Only fall back to formatted text if structured
                        // extraction fails, to avoid duplicate display.
                        if let Some(tp) = extract_tool_part_v2(p) {
                            tool_parts.push(tp);
                        } else {
                            let formatted = format_tool_part_v2(p);
                            if !formatted.is_empty() {
                                text_parts.push(formatted);
                            }
                        }
                    }
                    "tool-invocation" => {
                        if let Some(tp) = extract_tool_part_v1(p) {
                            tool_parts.push(tp);
                        } else {
                            let formatted = format_tool_part_v1(p);
                            if !formatted.is_empty() {
                                text_parts.push(formatted);
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
            text_parts.push(t.to_string());
        } else if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
            text_parts.push(content.to_string());
        }

        let text = text_parts.join("\n");
        if !text.is_empty() || !tool_parts.is_empty() {
            messages.push(StructuredMessage {
                role,
                text,
                tools: tool_parts,
            });
        }
    }

    Ok(messages)
}

/// Build an array of Slack `task_update` chunk JSON values from tool parts.
pub fn build_task_chunks(tools: &[ToolPart]) -> Vec<serde_json::Value> {
    tools.iter().map(|t| t.to_task_chunk()).collect()
}

// ── Fetch Assistant Messages ────────────────────────────────────────────

/// Fetch messages for a session, including assistant messages.
/// Used by the response batcher to collect AI responses for Slack relay.
pub async fn fetch_all_session_messages(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
) -> Result<Vec<(String, String)>> {
    let url = format!("{}/session/{}/message", base_url, session_id);

    let response = client
        .get(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to fetch session messages")?;

    let body: serde_json::Value = response.json().await?;

    let mut messages = Vec::new();
    let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.iter().collect()
    } else if let Some(obj) = body.as_object() {
        obj.values().collect()
    } else {
        vec![]
    };

    for item in items {
        let info = item.get("info");
        let role = info
            .and_then(|i| i.get("role"))
            .and_then(|r| r.as_str())
            .or_else(|| item.get("role").and_then(|r| r.as_str()))
            .unwrap_or("")
            .to_string();

        let text = if let Some(parts) = item.get("parts").and_then(|p| p.as_array()) {
            parts
                .iter()
                .filter_map(|p| {
                    let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match ptype {
                        "text" | "" => p
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string()),
                        "tool" => Some(format_tool_part_v2(p)),
                        "tool-invocation" => Some(format_tool_part_v1(p)),
                        _ => None,
                    }
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
            t.to_string()
        } else if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            continue;
        };

        if !text.is_empty() {
            messages.push((role, text));
        }
    }

    Ok(messages)
}

// ── Send User Message (non-system) ──────────────────────────────────────

/// Send a user message to a session asynchronously via the OpenCode API.
/// Uses `POST /session/{id}/prompt_async` with `system: "false"`.
pub async fn send_user_message(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
    text: &str,
) -> Result<()> {
    let url = format!("{}/session/{}/prompt_async", base_url, session_id);
    debug!(url, session_id, "Sending user message to session via Slack");

    let resp = client
        .post(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "system": "false",
            "parts": [{ "type": "text", "text": text }]
        }))
        .send()
        .await
        .context("Failed to send user message to opencode session")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "User message rejected by server: HTTP {} — {}",
            status,
            body
        );
    }

    Ok(())
}

/// Send a system message to a session (for thread replies).
pub async fn send_system_message(
    client: &reqwest::Client,
    base_url: &str,
    project_dir: &str,
    session_id: &str,
    text: &str,
) -> Result<()> {
    let url = format!("{}/session/{}/prompt_async", base_url, session_id);
    debug!(
        url,
        session_id, "Sending system message to session via Slack thread reply"
    );

    let resp = client
        .post(&url)
        .header("x-opencode-directory", project_dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "system": "true",
            "parts": [{ "type": "text", "text": text }]
        }))
        .send()
        .await
        .context("Failed to send system message to opencode session")?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "System message rejected by server: HTTP {} — {}",
            status,
            body
        );
    }

    Ok(())
}

// ── Slack Thread Slash Commands ─────────────────────────────────────────

/// Handle a slash command sent in a Slack thread.
///
/// Returns `true` if the text was recognized as a command (and handled),
/// `false` if it should be treated as a normal thread reply.
pub async fn handle_thread_slash_command(
    text: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
) -> bool {
    let trimmed = text.trim();
    let (cmd, args) = match trimmed.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (trimmed, ""),
    };

    match cmd {
        "@stop" => {
            do_stop_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                slack_state,
            )
            .await;
            true
        }
        "@watcher" => {
            do_watcher_command(
                channel,
                thread_ts,
                session_id,
                project_idx,
                project_dir,
                bot_token,
                base_url,
                slack_state,
                watcher_inserted,
                watcher_removed,
                args,
            )
            .await;
            true
        }
        "@compact" => {
            do_compact_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
        _ => false,
    }
}

/// `@stop` — Cancel the running OpenCode session (abort LLM generation and tool
/// execution).  The relay watcher and stream are left intact so that the final
/// state is still delivered to Slack once the session becomes idle.
async fn do_stop_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    _slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    // Abort the OpenCode session via server API.
    // This calls POST /session/{id}/abort which cancels any running LLM
    // generation and tool execution on the server side.
    let api = crate::api::ApiClient::new();
    let msg = match api.abort_session(base_url, project_dir, session_id).await {
        Ok(()) => {
            tracing::info!(
                "Slack @stop: aborted session {} via API",
                &session_id[..8.min(session_id.len())]
            );
            ":octagonal_sign: Session interrupted. The relay watcher will deliver any remaining output.".to_string()
        }
        Err(e) => {
            tracing::warn!(
                "Slack @stop: failed to abort session {}: {}",
                &session_id[..8.min(session_id.len())],
                e
            );
            format!(":warning: Failed to stop session: {}", e)
        }
    };
    let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
}

/// `@watcher` — Start or stop a continuation + hang protection watcher for this session.
///
/// The actual `WatcherConfig` insertion/removal happens inline in `app.rs`
/// (synchronous context with `&mut self` access).  The `watcher_inserted` and
/// `watcher_removed` flags tell us what happened so we can post the right
/// confirmation to Slack.  `args` is the subcommand text after `@watcher`
/// (e.g. "stop", "off", "remove", or "").
async fn do_watcher_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    _project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
    args: &str,
) {
    let client = reqwest::Client::new();

    let is_stop = matches!(args, "stop" | "off" | "remove");

    // --- Watcher removal case (`@watcher stop` / `off` / `remove`) ---
    if watcher_removed {
        let msg = ":no_entry_sign: Watcher removed for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // User asked to stop but no watcher was active.
    if is_stop {
        let msg = ":information_source: No watcher is active for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // --- Watcher insertion case (`@watcher` with no subcommand) ---
    if watcher_inserted {
        let msg =
            ":eyes: Watcher enabled for this thread.\n• Idle timeout: 15s\n• Hang detection: 180s";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
    } else {
        let msg = ":warning: Could not enable watcher — failed to acquire lock on session watchers. Please try again.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // Also ensure the relay watcher is running.
    {
        let s = slack_state.lock().await;
        if !s.relay_abort_handles.contains_key(session_id) {
            drop(s);
            // Need to re-spawn relay watcher.
            let handle = spawn_session_relay_watcher(
                session_id.to_string(),
                project_dir.to_string(),
                channel.to_string(),
                thread_ts.to_string(),
                bot_token.to_string(),
                base_url.to_string(),
                3,
                slack_state.clone(),
            );
            let mut s = slack_state.lock().await;
            s.relay_abort_handles
                .insert(session_id.to_string(), handle.abort_handle());
            tracing::info!(
                "Slack @watcher: re-spawned relay watcher for session {}",
                &session_id[..8.min(session_id.len())]
            );
        }
    }
}

/// `/compact` — Send a compaction/summarization system message to the session.
async fn do_compact_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    let client = reqwest::Client::new();

    let compact_msg = concat!(
        "SYSTEM: Summarize the conversation so far into a concise status update. ",
        "List what has been accomplished, what is currently in progress, and what remains to be done. ",
        "Then continue working on any pending tasks. ",
        "Keep the summary brief but include all relevant file paths and line numbers."
    );

    match send_system_message(&client, base_url, project_dir, session_id, compact_msg).await {
        Ok(()) => {
            let msg = ":recycle: Compaction triggered — session will summarize and continue.";
            let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
            tracing::info!(
                "Slack /compact: sent compaction message to session {}",
                &session_id[..8.min(session_id.len())]
            );
        }
        Err(e) => {
            let msg = format!(":x: Compaction failed: {}", e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            tracing::warn!(
                "Slack /compact: failed for session {}: {}",
                &session_id[..8.min(session_id.len())],
                e
            );
        }
    }
}

// ── Live Relay Watcher ──────────────────────────────────────────────────

/// Spawn a background task that polls a session for new messages every
/// `buffer_secs` and relays them to a Slack thread using streaming with
/// `task_update` chunks for tool progress.
///
/// Returns a `JoinHandle` whose `AbortHandle` can be stored to cancel later.
pub fn spawn_session_relay_watcher(
    session_id: String,
    project_dir: String,
    channel: String,
    thread_ts: String,
    bot_token: String,
    base_url: String,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
) -> tokio::task::JoinHandle<()> {
    spawn_session_relay_watcher_labeled(
        session_id,
        project_dir,
        channel,
        thread_ts,
        bot_token,
        base_url,
        buffer_secs,
        slack_state,
        None,
    )
}

/// Spawn a relay watcher with an optional label prefix for the first streamed
/// message (e.g. ":robot_face: **Subagent:**" for child sessions).
pub fn spawn_session_relay_watcher_labeled(
    session_id: String,
    project_dir: String,
    channel: String,
    thread_ts: String,
    bot_token: String,
    base_url: String,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
    label: Option<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let interval = Duration::from_secs(buffer_secs.max(1));
        let mut idle_polls: u32 = 0;
        const IDLE_STOP_THRESHOLD: u32 = 3;
        let mut last_streamed_role: Option<String> = None;
        let mut label_emitted = false;

        info!(
            "Slack relay watcher started for session {} (poll every {}s, label={:?})",
            &session_id[..8.min(session_id.len())],
            buffer_secs,
            label
        );

        loop {
            tokio::time::sleep(interval).await;

            // Read current offset.
            let offset = {
                let s = slack_state.lock().await;
                s.session_msg_offset.get(&session_id).copied().unwrap_or(0)
            };

            // Fetch messages with structured tool data from the OpenCode API.
            let messages = match fetch_session_messages_with_tools(
                &client,
                &base_url,
                &project_dir,
                &session_id,
            )
            .await
            {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::warn!(
                        "Slack relay watcher: failed to fetch messages for session {}: {}",
                        &session_id[..8.min(session_id.len())],
                        e
                    );
                    continue;
                }
            };

            let total_count = messages.len();
            if total_count <= offset {
                idle_polls += 1;
                // If the session has been idle long enough and we have an active
                // stream, finalize it so the Slack message stops "typing".
                if idle_polls >= IDLE_STOP_THRESHOLD {
                    let stream_ts = {
                        let s = slack_state.lock().await;
                        s.streaming_messages.get(&session_id).cloned()
                    };
                    if let Some(ref ts) = stream_ts {
                        tracing::info!(
                            "Slack relay: stopping stream {} for session {} (idle for {} polls)",
                            ts,
                            &session_id[..8.min(session_id.len())],
                            idle_polls
                        );
                        if let Err(e) = stop_stream(&client, &bot_token, &channel, ts).await {
                            tracing::warn!("Slack relay: stopStream failed: {}", e);
                        }
                        let mut s = slack_state.lock().await;
                        s.streaming_messages.remove(&session_id);
                    }
                }
                continue;
            }

            // We have new messages -- reset idle counter.
            idle_polls = 0;

            // Collect new messages.
            let new_messages: Vec<_> = messages
                .into_iter()
                .skip(offset)
                .filter(|m| !m.text.is_empty() || !m.tools.is_empty())
                .collect();

            if new_messages.is_empty() {
                // Offset still advances even if all were empty.
                let mut s = slack_state.lock().await;
                s.session_msg_offset.insert(session_id.clone(), total_count);
                continue;
            }

            tracing::info!(
                "Slack relay watcher: relaying {} new message(s) for session {} to thread {}",
                new_messages.len(),
                &session_id[..8.min(session_id.len())],
                thread_ts
            );

            // Separate text content and tool task_update chunks.
            let mut all_task_chunks: Vec<serde_json::Value> = Vec::new();
            let mut groups: Vec<(String, Vec<String>)> = Vec::new();

            for msg in &new_messages {
                // Collect task_update chunks from tool parts.
                if !msg.tools.is_empty() {
                    all_task_chunks.extend(build_task_chunks(&msg.tools));
                }

                // Group text by role for the markdown portion.
                if !msg.text.is_empty() {
                    let converted = markdown_to_slack_mrkdwn(&msg.text);
                    if let Some(last) = groups.last_mut() {
                        if last.0 == msg.role {
                            last.1.push(converted);
                            continue;
                        }
                    }
                    groups.push((msg.role.clone(), vec![converted]));
                }
            }

            let mut markdown_parts: Vec<String> = Vec::new();
            for (i, (role, texts)) in groups.iter().enumerate() {
                let body = texts.join("\n");
                // Skip divider + header for the first group if it continues
                // the same role from the previous poll cycle.
                let same_as_last = i == 0 && last_streamed_role.as_deref() == Some(role.as_str());
                if same_as_last {
                    markdown_parts.push(body);
                } else {
                    let role_upper = role.to_uppercase();
                    markdown_parts.push(format!(
                        "─────────────────────────────────────────────\n**{}:**\n{}",
                        role_upper, body
                    ));
                }
            }
            let mut relay_text = markdown_parts.join("\n");

            // Prepend optional label on first relay (e.g. subagent indicator).
            if !label_emitted {
                if let Some(ref lbl) = label {
                    relay_text = format!("{}\n{}", lbl, relay_text);
                }
                label_emitted = true;
            }
            // Update last_streamed_role to the final group's role.
            if let Some((role, _)) = groups.last() {
                last_streamed_role = Some(role.clone());
            }

            let has_task_chunks = !all_task_chunks.is_empty();

            // Check if we already have an active stream for this session.
            let active_stream_ts = {
                let s = slack_state.lock().await;
                s.streaming_messages.get(&session_id).cloned()
            };

            if let Some(ref stream_ts) = active_stream_ts {
                // Append to the existing stream with both text and task chunks.
                // appendStream has a 12k char limit for text; chunk if needed.
                let text_chunks = chunk_for_slack(&relay_text, 11_000);
                for (i, chunk) in text_chunks.iter().enumerate() {
                    // Attach task chunks only on the first text chunk to avoid
                    // duplicate task_update entries.
                    let chunks_for_append = if i == 0 && has_task_chunks {
                        Some(all_task_chunks.as_slice())
                    } else {
                        None
                    };
                    if let Err(e) = append_stream(
                        &client,
                        &bot_token,
                        &channel,
                        stream_ts,
                        chunk,
                        chunks_for_append,
                    )
                    .await
                    {
                        let err_str = format!("{}", e);
                        tracing::warn!("Slack relay: appendStream failed ({})", err_str,);

                        if err_str.contains("stream_mode_mismatch") {
                            // The stream was started without task_display_mode
                            // but we're now trying to append task_update chunks.
                            // Fix: stop the old stream and start a fresh one
                            // with proper task mode.
                            tracing::info!(
                                "Slack relay: restarting stream with task_display_mode for session {}",
                                &session_id[..8.min(session_id.len())]
                            );
                            let _ = stop_stream(&client, &bot_token, &channel, stream_ts).await;
                            {
                                let mut s = slack_state.lock().await;
                                s.streaming_messages.remove(&session_id);
                            }

                            // Gather remaining text chunks (current + rest).
                            let remaining_text = text_chunks[i..].join("");
                            let empty_chunks: Vec<serde_json::Value> = vec![];
                            let restart_chunks: &[serde_json::Value] = if has_task_chunks {
                                &all_task_chunks
                            } else {
                                &empty_chunks
                            };
                            match start_stream(
                                &client,
                                &bot_token,
                                &channel,
                                &thread_ts,
                                Some(&remaining_text),
                                Some(restart_chunks),
                                Some("timeline"),
                            )
                            .await
                            {
                                Ok(new_ts) => {
                                    tracing::info!(
                                        "Slack relay: restarted stream {} for session {}",
                                        new_ts,
                                        &session_id[..8.min(session_id.len())]
                                    );
                                    let mut s = slack_state.lock().await;
                                    s.streaming_messages.insert(session_id.clone(), new_ts);
                                }
                                Err(e2) => {
                                    tracing::warn!(
                                        "Slack relay: restart startStream also failed ({}), falling back to post",
                                        e2
                                    );
                                    let formatted = format!(
                                        "{}\n─────────────────────────────────────────────",
                                        remaining_text
                                    );
                                    let _ = post_message(
                                        &client,
                                        &bot_token,
                                        &channel,
                                        &formatted,
                                        Some(&thread_ts),
                                    )
                                    .await;
                                }
                            }
                        } else {
                            // Non-mode-mismatch error: stream may have been
                            // stopped externally; fall back to post.
                            let formatted =
                                format!("{}\n─────────────────────────────────────────────", chunk);
                            let _ = post_message(
                                &client,
                                &bot_token,
                                &channel,
                                &formatted,
                                Some(&thread_ts),
                            )
                            .await;
                            // Clear the stale stream reference.
                            let mut s = slack_state.lock().await;
                            s.streaming_messages.remove(&session_id);
                        }
                        break;
                    }
                }
            } else {
                // Always start with timeline display mode so that task_update
                // chunks can be appended later without a mode mismatch.
                // IMPORTANT: pass an empty chunks array (not None) even when
                // there are no task chunks yet — Slack only registers the
                // stream as task-capable when the `chunks` field is present
                // in the startStream call alongside `task_display_mode`.
                let empty_chunks: Vec<serde_json::Value> = vec![];
                let initial_chunks: &[serde_json::Value] = if has_task_chunks {
                    &all_task_chunks
                } else {
                    &empty_chunks
                };
                let display_mode = Some("timeline");
                match start_stream(
                    &client,
                    &bot_token,
                    &channel,
                    &thread_ts,
                    Some(&relay_text),
                    Some(initial_chunks),
                    display_mode,
                )
                .await
                {
                    Ok(stream_ts) => {
                        tracing::info!(
                            "Slack relay: started stream {} for session {} (task_chunks={})",
                            stream_ts,
                            &session_id[..8.min(session_id.len())],
                            all_task_chunks.len()
                        );
                        let mut s = slack_state.lock().await;
                        s.streaming_messages.insert(session_id.clone(), stream_ts);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Slack relay: startStream failed ({}), falling back to post",
                            e
                        );
                        // Fall back to regular posting.
                        let formatted = format!(
                            "{}\n─────────────────────────────────────────────",
                            relay_text
                        );
                        let post_chunks = chunk_for_slack(&formatted, 39_000);
                        for chunk in &post_chunks {
                            let _ = post_message(
                                &client,
                                &bot_token,
                                &channel,
                                chunk,
                                Some(&thread_ts),
                            )
                            .await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            }

            // Update offset and persist the session map.
            {
                let mut s = slack_state.lock().await;
                s.session_msg_offset.insert(session_id.clone(), total_count);
                // Persist to disk so we survive restarts.
                let map = SlackSessionMap {
                    session_threads: s.session_threads.clone(),
                    thread_sessions: s.thread_sessions.clone(),
                    msg_offsets: s.session_msg_offset.clone(),
                };
                if let Err(e) = map.save() {
                    tracing::warn!("Slack relay watcher: failed to save session map: {}", e);
                }
            }
        }
    })
}
