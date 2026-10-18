//! Embedded ACP HTTP server lifecycle.
use super::{config::AgentConfig, conn, routes, AcpEngine};
use crate::server::ServerHandle;
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::info;

/// Start an embedded server for one configured agent. Returns `(base_url, handle, engine)`
/// so the caller can subscribe to the raw event channel directly.
pub async fn start_embedded_server(
    id: &str,
    agent: AgentConfig,
    mcp: crate::mcp_registry::SharedRegistry,
) -> Result<(String, ServerHandle, Arc<AcpEngine>)> {
    let persist =
        dirs::config_dir().map(|d| d.join("opman").join(format!("acp_{id}_sessions.json")));
    let engine = Arc::new(AcpEngine::new(id.to_string(), agent, persist, mcp));

    // Discover the agent's models and modes off the startup path, so `/provider` serves a
    // real catalogue rather than an empty list the picker cannot repair.
    {
        let probing = engine.clone();
        let label = id.to_string();
        probing.probing.send_replace(true);
        tokio::spawn(async move {
            match conn::probe_capabilities(&probing).await {
                Ok(setup) => {
                    probing.set_capabilities(setup);
                    probing.note_started();
                }
                Err(e) => {
                    tracing::warn!(agent = %label, "ACP capability probe failed: {e:#}");
                    probing.note_failure(format!("{e:#}"));
                }
            }
            probing.probing.send_replace(false);
        });
    }

    let app = routes::router(engine.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .with_context(|| format!("failed to bind embedded ACP engine port for `{id}`"))?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    engine.set_url(&url);
    info!(agent = %id, %url, "ACP engine ready");

    let label = id.to_string();
    let serving = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(agent = %label, "ACP engine server error: {e}");
        }
    });
    if let Ok(mut slot) = engine.serve.lock() {
        *slot = Some(serving);
    }

    let handle: ServerHandle = Arc::new(std::sync::Mutex::new(None));
    Ok((url, handle, engine))
}
