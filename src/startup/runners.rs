use crate::{acp_engine, mcp_registry, runner, runner_lazy, runner_settings, session_disk};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
pub(super) struct Runners {
    pub registry: Arc<runner::RunnerRegistry>,
    pub mcp: mcp_registry::RegistryHandle,
    pub supervisor: Arc<acp_engine::supervisor::AcpSupervisor>,
    pub context: Arc<runner_lazy::LazyContext>,
}
pub(super) async fn boot(
    backend: crate::cli::AgentBackend,
    mcp_flags: mcp_registry::BuiltinFlags,
) -> Result<Runners> {
    // ACP agents are declared in config, so the set of runners is known only at runtime.
    // Register the ids before anything parses a runner label, or a perfectly valid agent
    // name would be rejected as unknown.
    let acp_config = acp_engine::config::load();
    runner::register_acp_runners(acp_config.active().map(|(id, _)| id.clone()));

    // Load after the ACP ids are registered, so `runners`/`excludeRunners` in mcp.json can
    // name an ACP agent, and after OPMAN_AGENT_MANAGER_SOCKET is published above, so the
    // agent-manager server's presence check resolves.
    let mcp_registry = mcp_registry::RegistryHandle::load(mcp_flags);
    // OpenCode's payload is handed to `opencode serve` once at spawn, so unlike the
    // other runners it does not pick up later `mcp.json` edits without a restart.
    let opencode_registry = mcp_registry.current();
    let opencode_config = mcp_registry::render::opencode_config(
        opencode_registry.for_runner(&runner::RunnerKind::Opencode),
        // OpenCode's config is process-wide: it is built once for `opencode serve`, so it
        // binds with the child's own working directory and no session id.
        opencode_registry.bind(".", None),
        mcp_flags,
    )
    .context("Failed to build OpenCode MCP configuration")?;

    // Keep the selected CLI as the TUI's default, but expose all available
    // runners through one registry for web sessions.  The adapters speak the
    // same REST-shaped contract, so switching runners does not leak protocol
    // details into handlers or the frontend.
    let default_runner = match backend {
        crate::cli::AgentBackend::Opencode => runner::RunnerKind::Opencode,
        crate::cli::AgentBackend::ClaudeCode => runner::RunnerKind::ClaudeCode,
        crate::cli::AgentBackend::ClaudeAcp => runner::RunnerKind::Claude,
    };
    // The `claude` slot is served by the generic ACP engine, so the agent behind it is
    // resolved now — a missing one is a misconfiguration worth reporting at boot, even
    // though the engine itself is not started until something uses it.
    let default_acp_agent = match backend {
        crate::cli::AgentBackend::ClaudeAcp => {
            let (id, agent) = acp_config
                .for_runner("claude")
                .context("No ACP agent is configured for the `claude` runner")?;
            Some((id.clone(), agent.clone()))
        }
        _ => None,
    };
    // Every runner call is now short — sends go to `prompt_async`, so nothing
    // here waits on an agent turn. A timeout means a wedged engine surfaces as
    // an error the UI can show instead of a request that hangs until the
    // browser or tunnel gives up.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    // Shared so a runner started later can register its child with the same Ctrl+C
    // handler and shutdown path as one started here.
    let lazy_ctx = runner_lazy::LazyContext::new();
    let opencode_config = Arc::new(opencode_config);

    // The built-in runners (OpenCode, Claude Code) come from the settings page's
    // `runners.json`: each is a lazy slot that starts its server on first use, or a
    // placeholder when switched off. The primary runner is the CLI's choice unless that
    // runner is switched off; its URL becomes the process-wide base URL when it starts.
    let builtin_factory = Arc::new(runner_settings::BuiltinFactory::new(
        lazy_ctx.clone(),
        mcp_registry.clone(),
        client.clone(),
        opencode_config.clone(),
    ));
    let runner_settings = Arc::new(runner_settings::RunnerSettings::boot(
        runner_settings::config_path(),
        builtin_factory,
        default_runner.clone(),
    ));
    let default_runner = runner_settings.primary().clone();
    let mut runner_impls: HashMap<runner::RunnerKind, Arc<dyn runner::Runner>> =
        runner_settings.slots().into_iter().collect();

    // The `claude` slot is served by the generic ACP engine. When it is the primary it is
    // built here rather than from `acp.json`, so its URL is published as the base URL.
    if let Some((id, agent)) =
        default_acp_agent.filter(|_| default_runner == runner::RunnerKind::Claude)
    {
        let kind = default_runner.clone();
        let mcp = mcp_registry.clone();
        let http = client.clone();
        let agent_id = id.clone();
        let primary = runner_lazy::LazyRunner::new(kind.clone(), lazy_ctx.clone(), move || {
            let kind = kind.clone();
            let mcp = mcp.clone();
            let http = http.clone();
            let agent_id = agent_id.clone();
            let agent = agent.clone();
            async move {
                let (url, handle, engine) =
                    acp_engine::start_embedded_server(&agent_id, agent, mcp)
                        .await
                        .with_context(|| format!("Failed to start ACP engine `{agent_id}`"))?;
                Ok(runner_lazy::LazyStart {
                    runner: Arc::new(runner::AcpRunner::new(
                        kind,
                        url.clone(),
                        http,
                        engine.clone(),
                    )),
                    handle: Some(handle),
                    engine: Some((agent_id, engine)),
                    base_url: Some(url),
                })
            }
        })
        .for_acp_agent(id.clone())
        .with_disk_sessions(session_disk::SessionStore::acp(&id));
        runner_impls.insert(default_runner.clone(), Arc::new(primary));
    }

    // Every configured ACP agent becomes a runner. This is the whole cost of adding one:
    // a config entry, no code. An agent that fails to start (missing command, bad args)
    // reports the failure on the send that asked for it, rather than at boot — which is
    // also what lets a binary installed while opman runs be picked up without a restart.
    for (id, agent) in acp_config.active() {
        let Some(kind) = runner::RunnerKind::parse(&agent.runner) else {
            tracing::warn!(agent = %id, runner = %agent.runner, "skipping ACP agent: unknown runner slot");
            continue;
        };
        if runner_impls.contains_key(&kind) {
            continue;
        }
        let agent_id = id.clone();
        let agent = agent.clone();
        let mcp = mcp_registry.clone();
        let http = client.clone();
        let slot = kind.clone();
        runner_impls.insert(
            kind.clone(),
            Arc::new(
                runner_lazy::LazyRunner::new(kind.clone(), lazy_ctx.clone(), move || {
                    let agent_id = agent_id.clone();
                    let agent = agent.clone();
                    let mcp = mcp.clone();
                    let http = http.clone();
                    let slot = slot.clone();
                    async move {
                        let (url, handle, engine) =
                            acp_engine::start_embedded_server(&agent_id, agent, mcp).await?;
                        Ok(runner_lazy::LazyStart {
                            runner: Arc::new(runner::AcpRunner::new(
                                slot,
                                url,
                                http,
                                engine.clone(),
                            )),
                            handle: Some(handle),
                            engine: Some((agent_id, engine)),
                            base_url: None,
                        })
                    }
                })
                .for_acp_agent(id.clone())
                .with_disk_sessions(session_disk::SessionStore::acp(id)),
            ),
        );
    }
    let runner_registry = Arc::new(runner::RunnerRegistry::new(default_runner, runner_impls));
    runner_registry.set_preferred(runner_settings.saved_default());
    runner_settings::install_global(runner_settings);

    // Hand the engines started above to the supervisor, so a later edit to `acp.json` can
    // start, restart or drop an agent against the same runner slots rather than fighting
    // for them. Only engines that actually made it into the registry are adopted.
    // Nothing is running yet: every ACP engine now joins the supervisor via `adopt_one`
    // at the moment its runner is first used.
    let acp_supervisor = Arc::new(acp_engine::supervisor::AcpSupervisor::adopt(
        runner_registry.clone(),
        mcp_registry.clone(),
        client.clone(),
        std::iter::empty(),
    ));
    // Both are needed by a runner that starts later: the registry to install the real
    // runner into and to fire the started-hook, the supervisor to take ownership of an
    // ACP engine so a later `acp.json` edit can reconcile against it.
    lazy_ctx.set_registry(&runner_registry);
    lazy_ctx.set_supervisor(&acp_supervisor);

    Ok(Runners {
        registry: runner_registry,
        mcp: mcp_registry,
        supervisor: acp_supervisor,
        context: lazy_ctx,
    })
}
