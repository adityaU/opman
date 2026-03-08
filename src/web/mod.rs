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
mod mcp_ws;
pub mod pty_manager;
mod sse;
mod static_files;
pub mod types;
mod tunnel;
mod web_state;

// Re-export public API used by main.rs
pub use types::ServerState;
pub use types::WebThemeColors;
pub use tunnel::{spawn_tunnel, TunnelHandle, TunnelMode, TunnelOptions};
pub use web_state::WebStateHandle;

use axum::routing::{delete, get, post};
use axum::Router;
use axum::extract::DefaultBodyLimit;
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
pub fn start_web_server(
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

    // Load config and create the independent web state
    let app_config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config for web state: {e}, using defaults");
        Config::default()
    });
    let web_state = WebStateHandle::new(&app_config, event_tx.clone(), raw_sse_tx.clone());
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
        .route("/project/add", post(handlers::add_project))
        .route("/project/remove", post(handlers::remove_project))
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
        // ── Context Window ───────────────────────────────────────────
        .route("/context-window", get(handlers::get_context_window))
        // ── File Edits / Diff Review ─────────────────────────────────
        .route(
            "/session/{session_id}/file-edits",
            get(handlers::get_file_edits),
        )
        // ── Cross-Session Search ─────────────────────────────────────
        .route(
            "/project/{project_idx}/search",
            get(handlers::search_messages),
        )
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
            "/session/{session_id}",
            delete(handlers::delete_session).patch(handlers::rename_session),
        )
        .route(
            "/session/{session_id}/command",
            post(handlers::execute_command),
        )
        .route(
            "/session/{session_id}/todos",
            get(handlers::get_session_todos),
        )
        // ── Multi-session dashboard ──────────────────────────────────
        .route("/sessions/overview", get(handlers::sessions_overview))
        .route("/sessions/tree", get(handlers::sessions_tree))
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
        .route("/session/events", get(sse::session_events_stream))
        // ── Git API (shell out to git CLI) ───────────────────────────
        .route("/git/status", get(handlers::git_status))
        .route("/git/diff", get(handlers::git_diff))
        .route("/git/log", get(handlers::git_log))
        .route("/git/stage", post(handlers::git_stage))
        .route("/git/unstage", post(handlers::git_unstage))
        .route("/git/commit", post(handlers::git_commit))
        .route("/git/discard", post(handlers::git_discard))
        .route("/git/show", get(handlers::git_show))
        .route("/git/branches", get(handlers::git_branches))
        .route("/git/checkout", post(handlers::git_checkout))
        .route("/git/range-diff", get(handlers::git_range_diff))
        .route("/git/context-summary", get(handlers::git_context_summary))
        // ── File browsing / editing ──────────────────────────────────
        .route("/agents", get(handlers::get_agents))
        .route("/files", get(handlers::browse_files))
        .route("/file/read", get(handlers::read_file))
        .route("/file/raw", get(handlers::read_file_raw))
        .route("/file/write", post(handlers::write_file))
        .route("/editor/lsp/diagnostics", get(handlers::editor_lsp_diagnostics))
        .route("/editor/lsp/hover", get(handlers::editor_lsp_hover))
        .route("/editor/lsp/definition", get(handlers::editor_lsp_definition))
        .route("/editor/lsp/format", post(handlers::editor_lsp_format))
        // ── Session Watcher ──────────────────────────────────────────
        .route("/watchers", get(handlers::list_watchers))
        .route("/watcher", post(handlers::create_watcher))
        .route("/watcher/sessions", get(handlers::get_watcher_sessions))
        .route(
            "/watcher/{session_id}",
            get(handlers::get_watcher).delete(handlers::delete_watcher),
        )
        .route("/watcher/{session_id}/messages", get(handlers::get_watcher_messages))
        // ── Session Continuity: Presence + Activity ──────────────────
        .route("/presence", get(handlers::get_presence).post(handlers::register_presence).delete(handlers::deregister_presence))
        .route("/activity", get(handlers::get_activity_feed))
        // ── Missions ───────────────────────────────────────────────
        .route("/missions", get(handlers::list_missions).post(handlers::create_mission))
        .route(
            "/missions/{mission_id}",
            axum::routing::patch(handlers::update_mission).delete(handlers::delete_mission),
        )
        // ── Personal Memory ─────────────────────────────────────
        .route("/memory", get(handlers::list_personal_memory).post(handlers::create_personal_memory))
        .route(
            "/memory/{memory_id}",
            axum::routing::patch(handlers::update_personal_memory).delete(handlers::delete_personal_memory),
        )
        // ── Autonomy Controls ──────────────────────────────────
        .route("/autonomy", get(handlers::get_autonomy_settings).post(handlers::update_autonomy_settings))
        // ── Routines ───────────────────────────────────────────
        .route("/routines", get(handlers::list_routines).post(handlers::create_routine))
        .route("/routines/{routine_id}", axum::routing::patch(handlers::update_routine).delete(handlers::delete_routine))
        .route("/routines/{routine_id}/run", post(handlers::run_routine))
        // ── Delegation Board ─────────────────────────────────
        .route("/delegation", get(handlers::list_delegated_work).post(handlers::create_delegated_work))
        .route("/delegation/{item_id}", axum::routing::patch(handlers::update_delegated_work).delete(handlers::delete_delegated_work))
        // ── Workspace Snapshots ─────────────────────────────────────
        .route("/workspaces", get(handlers::list_workspaces).post(handlers::save_workspace).delete(handlers::delete_workspace))
        // ── MCP WebSocket (AI agent tool bridge) ─────────────────────
        .route("/mcp/ws", get(mcp_ws::websocket_handler));

    Router::new()
        .route("/health", get(handlers::health))
        .nest("/api", api_routes)
        .fallback(static_files::serve)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB global body limit
        .with_state(state)
}
