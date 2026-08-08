//! Shared test-harness helpers for the web backend.
//!
//! Provides cheap, dependency-free constructors for [`ServerState`] and its
//! collaborators so handler / web-state tests can run against an in-memory
//! database with no background pollers, no real PTY manager, and no network.

#![cfg(test)]

use tokio::sync::broadcast;

use super::pty_manager::WebPtyHandle;
use super::types::{EditorEvent, ServerState, WebEvent};
use super::web_state::WebStateHandle;

/// A `WebPtyHandle` whose manager thread is never started. Any spawn command
/// sent through it fails fast with "PTY manager not running", which is exactly
/// what handler tests that don't exercise PTYs expect.
pub(crate) fn noop_pty_handle() -> WebPtyHandle {
    let (cmd_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    WebPtyHandle { cmd_tx }
}

/// Build a fully-formed [`ServerState`] backed by an in-memory database with no
/// authentication required (empty username/password) and no background tasks.
///
/// Must be called from within a tokio runtime (e.g. `#[tokio::test]`) because
/// [`crate::process_health::HealthHandle::new`] spawns a watchdog task.
pub(crate) fn test_server_state() -> ServerState {
    let (event_tx, _) = broadcast::channel::<WebEvent>(256);
    let (raw_sse_tx, _) = broadcast::channel::<String>(256);
    let (reload_tx, _) = broadcast::channel::<()>(4);
    let (editor_tx, _) = broadcast::channel::<EditorEvent>(64);
    let mut runners = std::collections::HashMap::new();
    runners.insert(
        crate::runner::RunnerKind::Opencode,
        std::sync::Arc::new(crate::runner::HttpRunner::new(
            crate::runner::RunnerKind::Opencode,
            "http://127.0.0.1:9",
            reqwest::Client::new(),
        )) as std::sync::Arc<dyn crate::runner::Runner>,
    );

    let mut web_state = WebStateHandle::new_test();
    web_state.set_editor_tx(editor_tx.clone());
    let runner_registry = std::sync::Arc::new(crate::runner::RunnerRegistry::new(
        crate::runner::RunnerKind::Opencode,
        runners,
    ));

    ServerState {
        web_state,
        jwt_secret: b"test-jwt-secret-0123456789abcdef".to_vec(),
        username: String::new(),
        password: String::new(),
        event_tx,
        raw_sse_tx,
        pty_mgr: noop_pty_handle(),
        http_client: reqwest::Client::new(),
        nvim_registry: crate::mcp::new_nvim_socket_registry(),
        lsp: std::sync::Arc::new(crate::lsp::LspPool::new()),
        skills_registry: crate::mcp_skills::SkillsRegistry::default(),
        mcp: crate::mcp_registry::RegistryHandle::default(),
        reload_tx,
        instance_name: None,
        backend: "opencode".to_string(),
        editor_tx,
        health: crate::process_health::HealthHandle::new(),
        internal_token: "test-internal-token".to_string(),
        acp: test_acp_supervisor(runner_registry.clone()),
        runner_registry,
        mcp_logins: std::sync::Arc::default(),
    }
}

/// A supervisor owning no engines, for handlers that only need the field to exist.
pub(crate) fn test_acp_supervisor(
    registry: std::sync::Arc<crate::runner::RunnerRegistry>,
) -> std::sync::Arc<crate::acp_engine::supervisor::AcpSupervisor> {
    std::sync::Arc::new(crate::acp_engine::supervisor::AcpSupervisor::adopt(
        registry,
        crate::mcp_registry::RegistryHandle::default(),
        reqwest::Client::new(),
        std::iter::empty(),
    ))
}

/// Like [`test_server_state`] but with credentials set, for auth tests.
pub(crate) fn test_server_state_with_auth(username: &str, password: &str) -> ServerState {
    let mut state = test_server_state();
    state.username = username.to_string();
    state.password = password.to_string();
    state
}

/// Build the production router around a test [`ServerState`].
pub(crate) fn test_router(state: ServerState) -> axum::Router {
    super::routes::build_router(state)
}

/// Point `mcp.json` and the OAuth token store at a temp directory.
///
/// Both, always: the MCP handlers read the config *and* the credential store to answer a
/// single listing, so redirecting only one of them still leaves a test writing into the
/// developer's real `~/.config/opman`.
pub(crate) struct ConfigRedirect {
    _tmp: tempfile::TempDir,
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    /// Shares the lock the other env-mutating tests use, so a second mutex cannot
    /// reintroduce the race this exists to prevent.
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl ConfigRedirect {
    pub(crate) fn new() -> Self {
        let guard = crate::claude_engine::claude_cli::ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let vars = [
            ("OPMAN_MCP_CONFIG", tmp.path().join("mcp.json")),
            // What `dirs::config_dir` reads, and therefore what moves the token store.
            ("XDG_CONFIG_HOME", tmp.path().to_path_buf()),
        ];
        let previous = vars
            .iter()
            .map(|(key, value)| {
                let prior = std::env::var_os(key);
                std::env::set_var(key, value);
                (*key, prior)
            })
            .collect();
        Self {
            _tmp: tmp,
            previous,
            _guard: guard,
        }
    }

    /// The `mcp.json` as it stands on disk right now.
    pub(crate) fn document(&self) -> crate::mcp_registry::config::McpConfig {
        crate::mcp_registry::config::load()
    }

    /// Write one server into the redirected `mcp.json`.
    pub(crate) fn declare(&self, name: &str, entry: crate::mcp_registry::config::ServerConfig) {
        let mut document = self.document();
        document.servers.insert(name.to_string(), entry);
        crate::mcp_registry::config::save(&document).expect("save mcp.json");
    }
}

impl Drop for ConfigRedirect {
    fn drop(&mut self) {
        for (key, prior) in self.previous.drain(..) {
            match prior {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// Run `fut` with a per-task opencode base-URL override in effect, so any
/// `crate::app::base_url()` call inside the awaited request future resolves to
/// `url` (a mock upstream). Isolated per tokio task — safe under parallel tests.
pub(crate) async fn scope_base_url<F>(url: String, fut: F) -> F::Output
where
    F: std::future::Future,
{
    crate::app::TEST_BASE_URL.scope(url, fut).await
}

/// Bind a mock "opencode" HTTP server to an ephemeral loopback port, serve
/// `router`, and return its base URL (e.g. `http://127.0.0.1:54321`). Pair with
/// [`scope_base_url`] so proxy handlers reach it instead of the dead default.
pub(crate) async fn start_mock_upstream(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://{addr}")
}

/// Send a request through the full router and return `(status, body_bytes)`.
///
/// `body` is sent as a JSON request body when `Some`. Exercises routing,
/// extractors and the handler together.
pub(crate) async fn send_json(
    router: axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (axum::http::StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let mut builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => {
            // A GET/DELETE with no body.
            builder = builder.header("content-type", "application/json");
            builder.body(Body::empty()).unwrap()
        }
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

#[cfg(test)]
mod harness_tests {
    use super::*;

    #[tokio::test]
    async fn builds_server_state() {
        let state = test_server_state();
        assert!(state.username.is_empty());
        assert_eq!(state.backend, "opencode");
        // web_state is usable and backed by an empty in-memory db.
        let paths = state.web_state.all_project_paths().await;
        assert!(paths.is_empty());
    }

    #[tokio::test]
    async fn mock_upstream_and_scope_work() {
        // A mock upstream that returns a message list; a proxy handler awaited
        // inside scope_base_url must reach it and return 200.
        use axum::routing::get;
        let mock = axum::Router::new().route(
            "/session/{id}/message",
            get(|| async { axum::Json(serde_json::json!([])) }),
        );
        let base = start_mock_upstream(mock).await;
        let state = test_server_state();
        // Seed a project so resolve_project_dir succeeds.
        state
            .web_state
            .add_project(std::env::temp_dir().to_str().unwrap(), Some("t"))
            .await
            .unwrap();
        let router = test_router(state);
        let (status, _body) = scope_base_url(
            base,
            send_json(router, "GET", "/api/session/s1/messages", None),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn builds_state_with_projects() {
        let ws = WebStateHandle::new_test_with_projects(vec![(
            "demo".to_string(),
            std::path::PathBuf::from("/tmp/demo"),
        )]);
        let paths = ws.all_project_paths().await;
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "/tmp/demo");
    }
}
