//! Building and stopping the built-in runners' slots.

use std::sync::Arc;

use anyhow::Context;

use super::BuiltinRunner;
use crate::runner::{HttpRunner, Runner};
use crate::runner_lazy::{DisabledRunner, LazyContext, LazyRunner, LazyStart};
use crate::session_disk::SessionStore;

/// What switching a built-in runner on or off needs from the outside world.
///
/// A trait so the switching logic can be tested without spawning `opencode` or touching
/// the process-wide base URL, which other tests running in parallel depend on.
pub trait SlotFactory: Send + Sync {
    /// Whether the runner's binary answers `--version`. Blocking; call off the reactor.
    fn installed(&self, runner: BuiltinRunner) -> bool;
    /// A slot that starts the runner on first use. `primary` publishes its URL as the
    /// process-wide base URL when it starts.
    fn build(&self, runner: BuiltinRunner, primary: bool) -> Arc<dyn Runner>;
    /// A slot that lists the runner's sessions and refuses everything else.
    fn disabled(&self, runner: BuiltinRunner) -> Arc<dyn Runner>;
    /// Stop whatever the runner has running. A runner that never started has nothing to
    /// stop, which is fine.
    fn stop(&self, runner: BuiltinRunner, primary: bool);
}

/// The real factory: `opencode serve` children and the embedded claude engine.
pub struct BuiltinFactory {
    ctx: Arc<LazyContext>,
    mcp: crate::mcp_registry::SharedRegistry,
    http: reqwest::Client,
    /// OpenCode's MCP payload, handed to `opencode serve` at spawn.
    opencode_config: Arc<String>,
}

impl BuiltinFactory {
    pub fn new(
        ctx: Arc<LazyContext>,
        mcp: crate::mcp_registry::SharedRegistry,
        http: reqwest::Client,
        opencode_config: Arc<String>,
    ) -> Self {
        Self {
            ctx,
            mcp,
            http,
            opencode_config,
        }
    }

    fn store(runner: BuiltinRunner) -> Option<SessionStore> {
        match runner {
            BuiltinRunner::Opencode => SessionStore::opencode(),
            BuiltinRunner::ClaudeCode => SessionStore::claude_engine(),
        }
    }
}

impl SlotFactory for BuiltinFactory {
    fn installed(&self, runner: BuiltinRunner) -> bool {
        std::process::Command::new(runner.binary())
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn build(&self, runner: BuiltinRunner, primary: bool) -> Arc<dyn Runner> {
        let kind = runner.kind();
        let mcp = self.mcp.clone();
        let http = self.http.clone();
        let config = self.opencode_config.clone();
        let start = move || {
            let kind = kind.clone();
            let mcp = mcp.clone();
            let http = http.clone();
            let config = config.clone();
            async move {
                let (url, handle) = match runner {
                    BuiltinRunner::Opencode => {
                        crate::server::spawn_agent_server(runner.backend(), Some(&config))
                            .context("Failed to start agent server")?
                    }
                    BuiltinRunner::ClaudeCode => {
                        crate::claude_engine::start_embedded_server(mcp)
                            .await
                            .context("Failed to start embedded claude engine")?
                    }
                };
                Ok(LazyStart {
                    runner: Arc::new(HttpRunner::new(kind, url.clone(), http)),
                    handle: Some(handle),
                    engine: None,
                    base_url: primary.then_some(url),
                })
            }
        };
        Arc::new(
            LazyRunner::new(runner.kind(), self.ctx.clone(), start)
                .with_disk_sessions(Self::store(runner)),
        )
    }

    fn disabled(&self, runner: BuiltinRunner) -> Arc<dyn Runner> {
        Arc::new(DisabledRunner::new(runner.kind(), Self::store(runner)))
    }

    fn stop(&self, runner: BuiltinRunner, primary: bool) {
        for handle in self.ctx.take_handles(&runner.kind()) {
            crate::server::kill_server(&handle);
        }
        if runner == BuiltinRunner::ClaudeCode {
            crate::claude_engine::shutdown_embedded();
        }
        if primary {
            crate::app::clear_base_url();
        }
    }
}
