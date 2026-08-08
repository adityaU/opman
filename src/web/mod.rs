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
pub mod keybindings;
mod mcp_ws;
pub mod pty_manager;
mod request_log;
mod routes;
mod runner_events;
pub mod session_instructions;
mod sse;
mod static_files;
#[cfg(test)]
pub(crate) mod test_support;
mod tunnel;
pub mod types;
mod web_state;

// Re-export public API used by main.rs
pub use tunnel::{spawn_tunnel, TunnelHandle, TunnelMode, TunnelOptions};
pub use types::ServerState;
pub use types::WebThemePair;
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
    /// Active agent backend name ("opencode" or "claude-code").
    pub backend: String,
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
    runner_registry: std::sync::Arc<crate::runner::RunnerRegistry>,
    mcp: crate::mcp_registry::RegistryHandle,
    acp: std::sync::Arc<crate::acp_engine::supervisor::AcpSupervisor>,
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

    // Random token guarding the loopback-only `/internal/*` Kanban API.
    let internal_token: String = format!("{:x}{:x}", rand::random::<u64>(), rand::random::<u64>());

    // Start the independent web PTY manager
    let pty_mgr = pty_manager::start_web_pty_manager();

    // Initialize skills registry
    let skills_registry = crate::mcp_skills::SkillsRegistry::default();
    *skills_registry.write().await = crate::mcp_skills::load_skills().await.unwrap_or_default();
    let (reload_tx, reload_rx) = broadcast::channel::<()>(1);
    crate::mcp_skills::spawn_skills_reload_watcher(reload_rx, skills_registry.clone());

    // Load config and create the independent web state
    let app_config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config for web state: {e}, using defaults");
        Config::default()
    });
    let mut web_state = WebStateHandle::new(
        &app_config,
        event_tx.clone(),
        raw_sse_tx.clone(),
        runner_registry.clone(),
    );
    web_state
        .set_default_runner(&runner_registry.default_kind().display_name())
        .await;
    // The web state owns the default runner's SSE listener. Forward events
    // from additional HTTP runners as well, so a handoff continues streaming
    // into the same browser connection.
    for (kind, endpoint) in runner_registry.event_endpoints() {
        if kind == runner_registry.default_kind() {
            continue;
        }
        runner_events::spawn_runner_event_forwarder(
            endpoint,
            kind.display_name().to_string(),
            raw_sse_tx.clone(),
            web_state.clone(),
        );
    }
    for (kind, receiver) in runner_registry.event_receivers() {
        runner_events::spawn_runner_event_receiver(
            receiver,
            kind.display_name().to_string(),
            raw_sse_tx.clone(),
            web_state.clone(),
        );
    }
    let (editor_tx, _) = broadcast::channel::<types::EditorEvent>(64);
    web_state.set_editor_tx(editor_tx.clone());
    let web_state_ret = web_state.clone();

    // Language servers for the editor. Only the editor handlers need these, so
    // the pool is owned here rather than threaded down from `App`. Servers are
    // started lazily on the first request for a file and reaped when idle.
    let lsp_pool = std::sync::Arc::new(crate::lsp::LspPool::new());
    crate::lsp::reaper::spawn(lsp_pool.clone());

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
        lsp: lsp_pool,
        skills_registry,
        mcp,
        reload_tx,
        instance_name: config.instance_name,
        backend: config.backend,
        editor_tx,
        health: crate::process_health::HealthHandle::new(),
        internal_token: internal_token.clone(),
        runner_registry,
        acp,
        mcp_logins: std::sync::Arc::default(),
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

    // Publish the internal API URL + token so the Kanban MCP server (spawned by
    // either backend) can reach the loopback-only `/internal/*` endpoints.
    write_internal_descriptor(actual_port, &internal_token);

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

/// Label a session the moment its own runner announces it.
///
/// `session.created` from a runner engine reaches web state before the creating
/// handler can record the runner, so without this the session is briefly
/// reported as belonging to the default runner. Insert-if-absent keeps explicit
/// labels (creation, handoff) authoritative.
async fn label_created_session(web_state: &WebStateHandle, data: &str, runner: &str) {
    let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else {
        return;
    };
    if event.get("type").and_then(serde_json::Value::as_str) != Some("session.created") {
        return;
    }
    let Some(session_id) = event
        .pointer("/properties/info/id")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    web_state
        .set_session_runner_if_absent(session_id, runner)
        .await;
}

/// Write `~/.config/opman/internal.json` = `{ "url": ..., "token": ... }`.
/// Read by `opman mcp-kanban` to call the internal Kanban API.
fn write_internal_descriptor(port: u16, token: &str) {
    let Some(dir) = dirs::config_dir().map(|d| d.join("opman")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let payload = serde_json::json!({
        "url": format!("http://127.0.0.1:{port}"),
        "token": token,
    });
    if let Ok(s) = serde_json::to_string_pretty(&payload) {
        let _ = std::fs::write(dir.join("internal.json"), s);
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;

#[cfg(test)]
#[path = "mod_startup_tests.rs"]
mod mod_startup_tests;
