//! Keeping the live ACP engines in step with `acp.json`.
//!
//! Adding an ACP server was already a config edit rather than a code change — but only one
//! read at startup, so it still cost a restart. This closes that gap: the supervisor owns
//! the engines it started and can be asked, at any point, to make the running set match a
//! freshly loaded config. An agent added from the settings page becomes a runner in the
//! same request; one removed stops being offered and its child processes are killed.
//!
//! Ownership is the rule that keeps this safe. The supervisor only ever installs or drops
//! runner slots it created — `opencode` and `claude-code` are served by other engines
//! entirely, and an ACP agent that names an occupied slot is reported as blocked rather
//! than allowed to displace whatever is there.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::config::{AcpConfig, AgentConfig};
use super::AcpEngine;
use crate::runner::{register_acp_runners, AcpRunner, RunnerKind, RunnerRegistry};

/// One agent opman started, and therefore owns.
struct Live {
    kind: RunnerKind,
    /// The definition this engine was started from. An edit that leaves it unchanged is
    /// not worth a restart; anything else is, because the launch command, environment and
    /// client capabilities are all fixed when the child is spawned.
    config: AgentConfig,
    engine: Arc<AcpEngine>,
}

/// What a reconcile changed.
///
/// Most of opman looks a runner up per request and so needs no telling. The SSE fan-out is
/// the exception: it subscribes once per runner, so a newly installed one needs its stream
/// attached or its output would never reach a browser.
#[derive(Debug, Default)]
pub struct AcpChanges {
    pub added: Vec<RunnerKind>,
    pub removed: Vec<RunnerKind>,
    /// Agent ids whose runner slot is held by an engine the supervisor does not own.
    pub blocked: Vec<String>,
    /// Agent ids whose new definition is on disk but not running: the default runner's
    /// engine cannot be restarted live. See [`retire`].
    pub deferred: Vec<String>,
}

/// The live ACP engines, reconcilable against config.
pub struct AcpSupervisor {
    registry: Arc<RunnerRegistry>,
    mcp: crate::mcp_registry::SharedRegistry,
    client: reqwest::Client,
    /// Keyed by agent id. Also the reconcile lock: starting and stopping processes must not
    /// interleave, so the whole pass runs under it.
    live: Mutex<HashMap<String, Live>>,
}

impl AcpSupervisor {
    /// Take ownership of engines that were started during boot.
    ///
    /// Startup builds the runner map before the registry exists, so those engines are
    /// adopted rather than started again — otherwise the first reconcile would see slots it
    /// did not create and refuse to touch them for the life of the process.
    pub fn adopt(
        registry: Arc<RunnerRegistry>,
        mcp: crate::mcp_registry::SharedRegistry,
        client: reqwest::Client,
        adopted: impl IntoIterator<Item = (RunnerKind, Arc<AcpEngine>)>,
    ) -> Self {
        let live = adopted
            .into_iter()
            .map(|(kind, engine)| {
                let entry = Live {
                    kind,
                    config: engine.agent.clone(),
                    engine,
                };
                (entry.engine.id.clone(), entry)
            })
            .collect();
        Self {
            registry,
            mcp,
            client,
            live: Mutex::new(live),
        }
    }

    /// Re-read `acp.json` and make the running set match it.
    pub async fn reload(&self) -> AcpChanges {
        self.reconcile(&super::config::load()).await
    }

    /// Make the running set match `cfg`.
    pub async fn reconcile(&self, cfg: &AcpConfig) -> AcpChanges {
        let mut live = self.live.lock().await;
        // Runner labels are parsed against this set, so a brand-new agent id has to be
        // registered before its slot can even be named.
        register_acp_runners(cfg.active().map(|(id, _)| id.clone()));

        let mut changes = AcpChanges::default();
        let (retired, deferred) = retire(&mut live, cfg, &self.registry.default_kind());
        changes.deferred = deferred;
        for entry in retired {
            self.registry.uninstall(&entry.kind);
            entry.engine.shutdown().await;
            tracing::info!(runner = %entry.kind.display_name(), "ACP agent stopped");
            changes.removed.push(entry.kind);
        }

        for (id, agent) in cfg.active() {
            if live.contains_key(id) {
                continue;
            }
            let Some(kind) = RunnerKind::parse(&agent.runner) else {
                tracing::warn!(agent = %id, runner = %agent.runner, "skipping ACP agent: unknown runner slot");
                continue;
            };
            if self.registry.has(&kind) {
                tracing::warn!(agent = %id, runner = %agent.runner, "ACP agent's runner slot is already served");
                changes.blocked.push(id.clone());
                continue;
            }
            match self.start(id, agent, kind.clone()).await {
                Ok(entry) => {
                    live.insert(id.clone(), entry);
                    changes.added.push(kind);
                }
                Err(error) => tracing::warn!(agent = %id, "ACP agent unavailable: {error}"),
            }
        }
        changes
    }

    /// Runner slots the supervisor is currently serving, keyed by agent id.
    pub async fn running(&self) -> HashMap<String, RunnerKind> {
        self.live
            .lock()
            .await
            .iter()
            .map(|(id, entry)| (id.clone(), entry.kind.clone()))
            .collect()
    }

    /// The agent serving opman's default runner, if that runner is an ACP agent at all.
    /// Its row is the one that cannot take an edit live.
    pub async fn default_agent(&self) -> Option<String> {
        let default = self.registry.default_kind();
        self.live
            .lock()
            .await
            .iter()
            .find(|(_, entry)| entry.kind == default)
            .map(|(id, _)| id.clone())
    }

    async fn start(&self, id: &str, agent: &AgentConfig, kind: RunnerKind) -> anyhow::Result<Live> {
        let (url, _handle, engine) =
            super::start_embedded_server(id, agent.clone(), self.mcp.clone()).await?;
        self.registry.install(
            kind.clone(),
            Arc::new(AcpRunner::new(
                kind.clone(),
                url,
                self.client.clone(),
                engine.clone(),
            )),
        );
        tracing::info!(agent = %id, runner = %kind.display_name(), "ACP agent started");
        Ok(Live {
            kind,
            config: agent.clone(),
            engine,
        })
    }
}

/// Pull out every live agent that `cfg` no longer describes the same way, sparing the one
/// serving `pinned`. Returns the agents to stop and the ids that were spared.
///
/// A restart is a retire followed by a start, which is why this runs to completion before
/// anything is started: the slot an edited agent is going back into is the one it is
/// vacating here.
///
/// `pinned` is opman's default runner, and it is the one engine that cannot be restarted.
/// Its URL was published to the TUI once at startup, so a new port would leave the TUI
/// talking to a closed socket. Its edit is on disk and takes effect on the next start —
/// reported rather than silently dropped, so the page can say so.
fn retire(
    live: &mut HashMap<String, Live>,
    cfg: &AcpConfig,
    pinned: &RunnerKind,
) -> (Vec<Live>, Vec<String>) {
    let mut stale = Vec::new();
    let mut deferred = Vec::new();
    for (id, entry) in live.iter() {
        let unchanged = cfg
            .agents
            .get(id.as_str())
            .is_some_and(|agent| agent.launchable() && &entry.config == agent);
        if unchanged {
            continue;
        }
        if &entry.kind == pinned {
            deferred.push(id.clone());
            continue;
        }
        stale.push(id.clone());
    }
    let retired = stale.into_iter().filter_map(|id| live.remove(&id)).collect();
    (retired, deferred)
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod supervisor_tests;
