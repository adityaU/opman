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
mod error;
mod handlers;
pub mod pty_manager;
mod sse;
mod static_files;
pub mod types;
mod tunnel;
mod web_state;

// Re-export public API used by main.rs
pub use types::ServerState;
pub use types::WebThemeColors;
pub use tunnel::{detect_tunnel_mode, spawn_tunnel, TunnelHandle};
pub use web_state::WebStateHandle;

use axum::routing::{get, post};
use axum::Router;
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
}

// ── Server startup ──────────────────────────────────────────────────

/// Start the fully independent web server in a background tokio task.
///
/// Returns `(actual_port, web_state_handle)`. The handle allows the TUI's
/// main loop to push theme changes into the web state (which broadcasts
/// them to connected SSE clients).
pub fn start_web_server(config: WebConfig) -> (u16, WebStateHandle) {
    let (event_tx, _event_rx) = broadcast::channel::<WebEvent>(1000);

    // Generate JWT secret (random per run — sessions don't survive restart)
    let jwt_secret: Vec<u8> = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32).map(|_| rng.gen::<u8>()).collect()
    };

    // Start the independent web PTY manager
    let pty_mgr = pty_manager::start_web_pty_manager();

    // Load config and create the independent web state
    let app_config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config for web state: {e}, using defaults");
        Config::default()
    });
    let web_state = WebStateHandle::new(&app_config, event_tx.clone());
    let web_state_ret = web_state.clone();

    let shared_state = ServerState {
        web_state,
        jwt_secret,
        username: config.username,
        password: config.password,
        event_tx,
        pty_mgr,
    };

    let app = build_router(shared_state);

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

// ── Router construction ─────────────────────────────────────────────

fn build_router(state: ServerState) -> Router {
    let api_routes = Router::new()
        // Auth
        .route("/auth/login", post(handlers::login))
        .route("/auth/verify", get(handlers::verify))
        // State
        .route("/state", get(handlers::get_state))
        .route(
            "/session/{session_id}/stats",
            get(handlers::get_session_stats),
        )
        .route("/theme", get(handlers::get_theme))
        .route("/themes", get(handlers::list_themes))
        .route("/theme/switch", post(handlers::switch_theme))
        // Actions (independent web state)
        .route("/project/switch", post(handlers::switch_project))
        .route("/session/select", post(handlers::select_session))
        .route("/session/new", post(handlers::new_session))
        .route("/panel/toggle", post(handlers::toggle_panel))
        .route("/panel/focus", post(handlers::focus_panel))
        // Web PTY management (independent from TUI)
        .route("/pty/spawn", post(handlers::spawn_pty))
        .route("/pty/write", post(handlers::pty_write))
        .route("/pty/resize", post(handlers::pty_resize))
        .route("/pty/kill", post(handlers::pty_kill))
        .route("/pty/list", get(handlers::pty_list))
        .route("/pty/stream", get(sse::terminal_stream))
        // App events SSE
        .route("/events", get(sse::events_stream))
        // ── Proxy endpoints (opencode server) ────────────────────────
        .route(
            "/session/{session_id}/messages",
            get(handlers::get_session_messages),
        )
        .route(
            "/session/{session_id}/message",
            post(handlers::send_message),
        )
        .route(
            "/session/{session_id}/abort",
            post(handlers::abort_session),
        )
        .route(
            "/session/{session_id}/command",
            post(handlers::execute_command),
        )
        .route(
            "/session/{session_id}/todos",
            get(handlers::get_session_todos),
        )
        .route("/providers", get(handlers::get_providers))
        .route("/commands", get(handlers::get_commands))
        .route(
            "/permission/{request_id}/reply",
            post(handlers::reply_permission),
        )
        .route(
            "/question/{request_id}/reply",
            post(handlers::reply_question),
        )
        // Session events SSE (proxied from opencode)
        .route("/session/events", get(sse::session_events_stream));

    Router::new()
        .nest("/api", api_routes)
        .fallback(static_files::serve)
        .with_state(state)
}
