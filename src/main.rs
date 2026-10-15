#![deny(unsafe_code)]

mod acp_engine;
mod api;
mod app;
mod background_tasks;
mod blockkit;
mod browser;
mod claude_engine;
mod cli;
mod cli_skills;
mod command_palette;
mod config;
mod event_input;
mod event_loop;
mod event_mouse;
mod input;
mod integrations;
mod ipc;
mod loopback;
mod lsp;
mod mcp;
mod mcp_agent_manager;
mod mcp_ask;
mod mcp_browser;
mod mcp_kanban;
mod mcp_neovim;
mod mcp_oauth;
mod mcp_probe;
mod mcp_proxy;
mod mcp_registry;
mod mcp_skills;
mod mcp_time;
mod mcp_ui;
mod mouse_handler;
mod nvim_rpc;
mod preflight;
mod process_health;
mod pty;
mod remote;
mod runner;
mod runner_handoff;
mod runner_lazy;
mod runner_limits;
mod runner_settings;
mod server;
mod session_disk;
use integrations::slack;
mod setup;
mod sse;
mod theme;
mod theme_gen;
mod todo_db;
mod ui;
mod usage;
mod util;
mod vim_mode;
mod web;
mod which_key;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tracing::info;

use crate::app::{App, BackgroundEvent};
use crate::cli::{Cli, Commands};
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Always log to file: <config_dir>/opman/opman.log
    let log_dir = dirs::config_dir()
        .expect("Could not determine config directory")
        .join("opman");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");
    let log_path = log_dir.join("opman.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("Failed to open log file");
    let log_writer: Box<dyn io::Write + Send> = Box::new(log_file);
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(log_writer))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,notify=error,walkdir=error")
            }),
        )
        .init();

    // ── Parse CLI arguments ─────────────────────────────────────────
    let cli = Cli::parse();

    // ── Handle subcommands (early exit) ──────────────────────────────
    match cli.command {
        Some(Commands::Mcp { project_path }) => {
            return mcp::run_mcp_bridge(project_path.unwrap_or_else(|| PathBuf::from("."))).await;
        }
        Some(Commands::McpProxy { name }) => {
            mcp_proxy::run_mcp_proxy(&name).await?;
            return Ok(());
        }
        Some(Commands::McpSkills) => {
            mcp_skills::bridge::run_mcp_skills_bridge().await?;
            return Ok(());
        }
        Some(Commands::McpTime) => {
            return mcp_time::run_mcp_time_bridge().await;
        }
        Some(Commands::McpUi) => {
            return mcp_ui::run_mcp_ui_bridge().await;
        }
        Some(Commands::McpKanban) => {
            return mcp_kanban::run_mcp_kanban_bridge().await;
        }
        Some(Commands::McpBrowser { project_path }) => {
            return mcp_browser::run_mcp_browser_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await;
        }
        Some(Commands::McpAsk { project_path }) => {
            return mcp_ask::run_mcp_ask_bridge(project_path.unwrap_or_else(|| PathBuf::from(".")))
                .await;
        }
        Some(Commands::McpAgentManager { project_path }) => {
            return mcp_agent_manager::run_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await;
        }
        Some(Commands::ClaudeHook) => {
            return claude_engine::run_permission_hook().await;
        }
        Some(Commands::McpNvim { project_path }) => {
            return mcp_neovim::run_mcp_neovim_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await;
        }
        Some(Commands::SlackManifest) => {
            return setup::handle_slack_manifest();
        }
        Some(Commands::Skills { subcommand }) => {
            return cli_skills::handle_skills(subcommand).await;
        }
        None => {} // Default mode: run the TUI
    }

    // ── Validate CLI argument combinations ───────────────────────────
    if let Err(msg) = cli.validate() {
        eprintln!("error: {msg}");
        std::process::exit(2);
    }

    // ── Derive computed flags ────────────────────────────────────────
    let backend = cli.resolved_backend();
    let enable_web = cli.enable_web();
    let tunnel_mode = cli.tunnel_mode();

    let mcp_flags = cli.builtin_mcp();
    let enable_any_mcp = mcp_flags.any();

    // Publish the path before starting any external runner process; OpenCode
    // serve inherits its environment and later launches the MCP child.
    let agent_manager_socket = mcp_agent_manager::socket_path();
    std::env::set_var(
        mcp_agent_manager::SOCKET_ENV,
        agent_manager_socket.to_string_lossy().as_ref(),
    );

    let web_addr = std::net::SocketAddr::new(cli.web_bind, cli.web_port.unwrap_or(0));
    let cli_browser_mode = cli.browser_mode();
    let web_user = cli.web_user.unwrap_or_default();
    let web_pass = cli.web_pass.unwrap_or_default();
    let web_only = cli.web_only;
    let exit_on_stdin_eof = cli.exit_on_stdin_eof;

    // Derive instance name from --tunnel-hostname for the web UI page title.
    // e.g. "myapp.example.com" → "Myapp", "example.com" → "Example"
    // The result is title-cased (first letter of each segment capitalised).
    let instance_name: Option<String> = cli.tunnel_hostname.as_deref().and_then(|h| {
        let h = h.trim();
        if h.is_empty() {
            return None;
        }
        let parts: Vec<&str> = h.split('.').collect();
        let raw = match parts.len() {
            0 => return None,
            1 => parts[0].to_string(),               // bare name, no dots
            2 => parts[0].to_string(),               // "example.com" → "example"
            _ => parts[..parts.len() - 2].join("."), // "myapp.example.com" → "myapp"
        };
        // Title-case: capitalise first letter of each dot-separated segment
        let title = raw
            .split('.')
            .map(|seg| {
                let mut c = seg.chars();
                match c.next() {
                    None => String::new(),
                    Some(first) => {
                        let upper: String = first.to_uppercase().collect();
                        upper + c.as_str()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(".");
        Some(title)
    });

    info!("opman starting");

    // Ensure required Docker containers (e.g. SearXNG) are running in background
    preflight::spawn_container_checks();

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
    if let Some((id, agent)) = default_acp_agent.filter(|_| default_runner == runner::RunnerKind::Claude) {
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
                let (url, handle, engine) = acp_engine::start_embedded_server(&agent_id, agent, mcp)
                    .await
                    .with_context(|| format!("Failed to start ACP engine `{agent_id}`"))?;
                Ok(runner_lazy::LazyStart {
                    runner: Arc::new(runner::AcpRunner::new(kind, url.clone(), http, engine.clone())),
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

    // The agent-manager MCP is attached to every runner.  MCP child processes
    // inherit this per-opman socket path and use it to reach the shared
    // registry, so cross-runner sends do not depend on which runner spawned
    // the child.
    let agent_manager_socket = mcp_agent_manager::spawn(runner_registry.clone())
        .context("Failed to start agent manager MCP")?;
    std::env::set_var(
        mcp_agent_manager::SOCKET_ENV,
        agent_manager_socket.to_string_lossy().as_ref(),
    );

    // Kill the server on Ctrl+C (even if the TUI hasn't reached cleanup)
    {
        let handles = lazy_ctx.clone();
        ctrlc::set_handler(move || {
            for handle in &handles.handles() {
                server::kill_server(handle);
            }
            std::process::exit(0);
        })
        .ok();
    }

    let config = Config::load().context("Failed to load config")?;

    // Create background event channel and app state
    let (bg_tx, bg_rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let mut app = App::new(config, bg_tx.clone());

    // Generate theme files for PTY programs (neovim, zsh, gitui)
    if let Err(e) = theme_gen::write_theme_files(&app.theme) {
        tracing::warn!("Failed to write theme files: {}", e);
    }

    // Start web UI server (if enabled)
    let (web_actual_port, web_state_handle) = setup::setup_web_server(
        enable_web,
        web_addr,
        &web_user,
        &web_pass,
        instance_name,
        backend.display_name(),
        cli_browser_mode,
        runner_registry.clone(),
        mcp_registry.clone(),
        acp_supervisor,
    )
    .await?;

    // Make the web state handle available to the TUI (e.g. for routine panel)
    if let Some(ref wsh) = web_state_handle {
        app.web_state = Some(wsh.clone());

        // Spawn a listener that pushes web-state events (e.g. RoutineUpdated)
        // into the TUI background event channel so overlays refresh automatically.
        let wsh2 = wsh.clone();
        let bg_tx2 = bg_tx.clone();
        tokio::spawn(async move {
            let mut rx = wsh2.subscribe_events();
            loop {
                match rx.recv().await {
                    Ok(crate::web::types::WebEvent::RoutineUpdated) => {
                        let (defs, _) = wsh2.list_routines().await;
                        let routines: Vec<crate::app::RoutineItem> = defs
                            .iter()
                            .map(crate::app::RoutineItem::from_definition)
                            .collect();
                        let _ =
                            bg_tx2.send(crate::app::BackgroundEvent::RoutinesFetched { routines });
                    }
                    Ok(_) => {} // Ignore other web events for now
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!("Web event listener lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // ── Spawn Cloudflare tunnel if configured ────────────────────────
    let _tunnel_handle: Option<web::TunnelHandle> = if enable_web {
        if let Some(mode) = tunnel_mode {
            let tunnel_opts = web::TunnelOptions {
                protocol: cli.tunnel_protocol.clone(),
                region: cli.tunnel_region.clone(),
                edge_ips: cli.tunnel_edge_ip.clone(),
            };
            Some(web::spawn_tunnel(mode, web_actual_port, &tunnel_opts).await)
        } else {
            None
        }
    } else {
        None
    };

    // ── web-only mode: skip the TUI entirely, run headless ────────
    if web_only {
        info!("Running in web-only mode (no TUI)");

        // Still need MCP + session setup (headless=true skips PTY spawning)
        setup::setup_initial_projects(&mut app, mcp_flags, true);

        println!(
            "opman web-only mode — web UI at http://localhost:{}",
            web_actual_port
        );
        if _tunnel_handle.is_some() {
            println!("  (also exposed via Cloudflare tunnel — see URL above)");
        }
        println!("Press Ctrl+C to stop.");
        if exit_on_stdin_eof {
            let (mut stdin, mut sink) = (tokio::io::stdin(), tokio::io::sink());
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = tokio::io::copy(&mut stdin, &mut sink) => {
                    info!("stdin closed; parent is gone");
                }
            }
        } else {
            tokio::signal::ctrl_c().await.ok();
        }

        // Clean up MCP socket files
        if enable_any_mcp {
            for project in &app.projects {
                mcp::cleanup_socket(&project.path);
            }
        }

        for handle in &lazy_ctx.handles() {
            server::kill_server(handle);
        }
        info!("opman shut down (web-only)");
        return Ok(());
    }

    // Setup terminal — TUI renders IMMEDIATELY after this
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    stdout
        .execute(EnableMouseCapture)
        .context("Failed to enable mouse capture")?;
    stdout
        .execute(EnableBracketedPaste)
        .context("Failed to enable bracketed paste")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Setup KV file watcher for theme reloading
    let (watcher_rx, _watcher) = setup::setup_kv_watcher()?;

    // Kick off initial data loading for all projects
    setup::setup_initial_projects(&mut app, mcp_flags, false);

    // Start Slack integration if enabled
    setup::setup_slack(&mut app);

    // Main event loop — TUI renders on first iteration (instant startup!)
    let result =
        event_loop::run_event_loop(&mut terminal, &mut app, watcher_rx, bg_rx, web_state_handle)
            .await;

    // Cleanup (always runs, even if event loop errored)
    disable_raw_mode().ok();
    terminal.backend_mut().execute(DisableBracketedPaste).ok();
    terminal.backend_mut().execute(DisableMouseCapture).ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    server::shutdown_all_ptys(&mut app.projects);
    for handle in &lazy_ctx.handles() {
        server::kill_server(handle);
    }

    for child in app.popout_windows.drain(..) {
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();
    }

    // Clean up MCP socket files
    if enable_any_mcp {
        for project in &app.projects {
            mcp::cleanup_socket(&project.path);
        }
    }

    info!("opman shut down");

    result
}
