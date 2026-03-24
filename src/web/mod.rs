//! Web UI server for opman.
//!
//! Runs an Axum HTTP server that is **fully independent** of the TUI, exposing:
//! - Embedded React frontend (via rust-embed)
//! - REST API for state queries and actions
//! - SSE streams for real-time terminal output and app events
//! - JWT-based authentication
//! - Independent web-owned PTY instances (shell, neovim, gitui, opencode)
//!
//! ## Architecture
//!
//! The web server maintains its own state via `WebStateHandle`, which:
//! - Loads projects from `Config::load()`
//! - Polls the `opencode serve` REST API for sessions
//! - Listens to the opencode SSE `/event` stream for stats and busy/idle
//! - Stores panel visibility, focused panel, active project
//!
//! Terminal I/O is handled by the `WebPtyManager` which owns independent
//! PTY instances — completely separate from the TUI's PTYs. Raw PTY
//! output bytes are streamed to xterm.js via SSE for native rendering.
//!
//! No `WebRequest` channel, no TUI event loop dependency.
//!
//! ## Module layout
//!
//! - `types` — Serializable API types, shared server state
//! - `error` — Unified `WebError` implementing `IntoResponse`
//! - `auth`  — JWT creation/verification and `AuthUser` extractor
//! - `handlers` — REST API route handlers
//! - `sse`  — SSE streaming (terminal output + app events)
//! - `web_state` — Independent state manager (talks to opencode API directly)
//! - `static_files` — Embedded React frontend serving
//! - `pty_manager` — Independent web-owned PTY instances

mod auth;
pub(crate) mod db;
mod error;
mod handlers;
mod mcp_ws;
pub mod pty_manager;
mod routes;
mod sse;
mod static_files;
pub mod types;
mod tunnel;
mod web_state;

// Re-export public API used by main.rs
pub use types::ServerState;
pub use types::WebThemePair;
pub use tunnel::{spawn_tunnel, TunnelHandle, TunnelMode, TunnelOptions};
pub use web_state::WebStateHandle;

use tokio::sync::broadcast;
use tracing::{error, info};

use crate::config::Config;
use types::WebEvent;

// ── Public configuration ────────────────────────────────────────────

/// Configuration for the web server, parsed from CLI args / env vars.
pub struct WebConfig {
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    /// Optional instance name (from tunnel subdomain/name) used as page title.
    pub instance_name: Option<String>,
}

// ── Server startup ──────────────────────────────────────────────────

/// Start the fully independent web server in a background tokio task.
///
/// Returns `(actual_port, web_state_handle)`. The handle allows the TUI's
/// main loop to push theme changes into the web state (which broadcasts
/// them to connected SSE clients).
pub async fn start_web_server(
    config: WebConfig,
    nvim_registry: crate::mcp::NvimSocketRegistry,
) -> (u16, WebStateHandle) {
    let (event_tx, _event_rx) = broadcast::channel::<WebEvent>(1000);
    // Raw upstream SSE events — re-broadcast to web clients so we don't need
    // a separate upstream connection per browser tab.
    let (raw_sse_tx, _) = broadcast::channel::<String>(2000);

    // Generate JWT secret (random per run — sessions don't survive restart)
    let jwt_secret: Vec<u8> = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32).map(|_| rng.gen::<u8>()).collect()
    };

    // Start the independent web PTY manager
    let pty_mgr = pty_manager::start_web_pty_manager();

    // Initialize skills registry
    let skills_registry = crate::mcp_skills::SkillsRegistry::default();
    *skills_registry.write().await = crate::mcp_skills::load_skills().await.unwrap_or_default();
    let (reload_tx, reload_rx) = broadcast::channel::<()>(1);
    crate::mcp_skills::spawn_mcp_skills_server(reload_rx, skills_registry.clone());

    // Load config and create the independent web state
    let app_config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config for web state: {e}, using defaults");
        Config::default()
    });
    let mut web_state = WebStateHandle::new(&app_config, event_tx.clone(), raw_sse_tx.clone());
    let (editor_tx, _) = broadcast::channel::<types::EditorEvent>(64);
    web_state.set_editor_tx(editor_tx.clone());
    let web_state_ret = web_state.clone();

    let shared_state = ServerState {
        web_state,
        jwt_secret,
        username: config.username,
        password: config.password,
        event_tx,
        raw_sse_tx,
        pty_mgr,
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()),
        nvim_registry,
        skills_registry,
        reload_tx,
        instance_name: config.instance_name,
        editor_tx,
        health: crate::process_health::HealthHandle::new(),
    };

    let app = routes::build_router(shared_state);

    // Bind to port (0 = random available port)
    let port = config.port.unwrap_or(0);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    // Bind synchronously to discover the actual port before returning
    let listener = std::net::TcpListener::bind(addr)
        .unwrap_or_else(|e| panic!("Failed to bind web server to port {port}: {e}"));
    let actual_port = listener
        .local_addr()
        .expect("Failed to get local address")
        .port();
    listener.set_nonblocking(true).ok();

    let tokio_listener = tokio::net::TcpListener::from_std(listener)
        .expect("Failed to convert std TcpListener to tokio");

    // Spawn the server in a background task
    tokio::spawn(async move {
        info!("Web UI server listening on port {}", actual_port);
        if let Err(e) = axum::serve(tokio_listener, app).await {
            error!("Web server error: {}", e);
        }
    });

    (actual_port, web_state_ret)
}
