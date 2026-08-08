#![deny(unsafe_code)]

mod acp_engine;
mod api;
mod app;
mod background_tasks;
mod blockkit;
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
mod loopback;
mod lsp;
mod mcp;
mod mcp_agent_manager;
mod mcp_ask;
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
mod runner;
mod runner_handoff;
mod server;
use integrations::slack;
mod setup;
mod sse;
mod theme;
mod theme_gen;
mod todo_db;
mod ui;
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
            return mcp::run_mcp_bridge(project_path.unwrap_or_else(|| PathBuf::from(".")))
                .await
                .map_err(Into::into);
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
            return mcp_time::run_mcp_time_bridge().await.map_err(Into::into);
        }
        Some(Commands::McpUi) => {
            return mcp_ui::run_mcp_ui_bridge().await.map_err(Into::into);
        }
        Some(Commands::McpKanban) => {
            return mcp_kanban::run_mcp_kanban_bridge()
                .await
                .map_err(Into::into);
        }
        Some(Commands::McpAsk { project_path }) => {
            return mcp_ask::run_mcp_ask_bridge(project_path.unwrap_or_else(|| PathBuf::from(".")))
                .await
                .map_err(Into::into);
        }
        Some(Commands::McpAgentManager { project_path }) => {
            return mcp_agent_manager::run_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await
            .map_err(Into::into);
        }
        Some(Commands::ClaudeHook) => {
            return claude_engine::run_permission_hook()
                .await
                .map_err(Into::into);
        }
        Some(Commands::McpNvim { project_path }) => {
            return mcp_neovim::run_mcp_neovim_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await
            .map_err(Into::into);
        }
        Some(Commands::SlackManifest) => {
            return setup::handle_slack_manifest();
        }
        Some(Commands::Skills { subcommand }) => {
            return cli_skills::handle_skills(subcommand)
                .await
                .map_err(Into::into);
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
        "OPMAN_AGENT_MANAGER_SOCKET",
        agent_manager_socket.to_string_lossy().as_ref(),
    );

    let web_port = cli.web_port;
    let web_user = cli.web_user.unwrap_or_default();
    let web_pass = cli.web_pass.unwrap_or_default();
    let web_only = cli.web_only;

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

    // Start the agent backend on a free port. opencode runs as an external
    // `opencode serve` process; claude-code is served by an in-process adapter
    // that speaks the same opencode REST + SSE contract (backed by `claude`
    // background agents); the `claude` slot is served by the generic ACP engine.
    let mut acp_engines: HashMap<runner::RunnerKind, Arc<acp_engine::AcpEngine>> = HashMap::new();
    let (base_url, server_handle) = if backend == crate::cli::AgentBackend::ClaudeCode {
        claude_engine::start_embedded_server(mcp_registry.clone())
            .await
            .context("Failed to start embedded claude engine")?
    } else if backend == crate::cli::AgentBackend::ClaudeAcp {
        let (id, agent) = acp_config
            .for_runner("claude")
            .context("No ACP agent is configured for the `claude` runner")?;
        let (url, handle, engine) =
            acp_engine::start_embedded_server(id, agent.clone(), mcp_registry.clone())
                .await
                .with_context(|| format!("Failed to start ACP engine `{id}`"))?;
        acp_engines.insert(runner::RunnerKind::Claude, engine);
        (url, handle)
    } else {
        server::spawn_agent_server(backend, Some(&opencode_config))
            .context("Failed to start agent server")?
    };
    crate::app::init_base_url(base_url);

    // Keep the selected CLI as the TUI's default, but expose all available
    // runners through one registry for web sessions.  The adapters speak the
    // same REST-shaped contract, so switching runners does not leak protocol
    // details into handlers or the frontend.
    let default_runner = match backend {
        crate::cli::AgentBackend::Opencode => runner::RunnerKind::Opencode,
        crate::cli::AgentBackend::ClaudeCode => runner::RunnerKind::ClaudeCode,
        crate::cli::AgentBackend::ClaudeAcp => runner::RunnerKind::Claude,
    };
    // Every runner call is now short — sends go to `prompt_async`, so nothing
    // here waits on an agent turn. A timeout means a wedged engine surfaces as
    // an error the UI can show instead of a request that hangs until the
    // browser or tunnel gives up.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut runner_impls: HashMap<runner::RunnerKind, Arc<dyn runner::Runner>> = HashMap::new();
    runner_impls.insert(
        default_runner.clone(),
        Arc::new(runner::HttpRunner::new(
            default_runner.clone(),
            crate::app::base_url(),
            client.clone(),
        )),
    );
    let mut server_handles = vec![server_handle];

    // Start the other HTTP-backed runner when its executable/adapter is
    // available. Missing optional binaries simply make that runner unavailable
    // in the picker instead of preventing opman from starting.
    if !runner_impls.contains_key(&runner::RunnerKind::Opencode)
        && std::process::Command::new("opencode")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    {
        if let Ok((url, handle)) =
            server::spawn_agent_server(crate::cli::AgentBackend::Opencode, Some(&opencode_config))
        {
            runner_impls.insert(
                runner::RunnerKind::Opencode,
                Arc::new(runner::HttpRunner::new(
                    runner::RunnerKind::Opencode,
                    url,
                    client.clone(),
                )),
            );
            server_handles.push(handle);
        }
    }
    let claude_bin = std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
    if std::process::Command::new(&claude_bin)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        if !runner_impls.contains_key(&runner::RunnerKind::ClaudeCode) {
            if let Ok((url, handle)) =
                claude_engine::start_embedded_server(mcp_registry.clone()).await
            {
                runner_impls.insert(
                    runner::RunnerKind::ClaudeCode,
                    Arc::new(runner::HttpRunner::new(
                        runner::RunnerKind::ClaudeCode,
                        url,
                        client.clone(),
                    )),
                );
                server_handles.push(handle);
            }
        }
    }

    // Every configured ACP agent becomes a runner. This is the whole cost of adding one:
    // a config entry, no code. An agent that fails to start (missing command, bad args)
    // simply does not appear in the picker, exactly like a missing optional binary.
    for (id, agent) in acp_config.active() {
        let Some(kind) = runner::RunnerKind::parse(&agent.runner) else {
            tracing::warn!(agent = %id, runner = %agent.runner, "skipping ACP agent: unknown runner slot");
            continue;
        };
        if runner_impls.contains_key(&kind) {
            continue;
        }
        let engine = match acp_engines.get(&kind) {
            Some(engine) => {
                runner_impls.insert(
                    kind.clone(),
                    Arc::new(runner::AcpRunner::new(
                        kind.clone(),
                        engine.url(),
                        client.clone(),
                        engine.clone(),
                    )),
                );
                continue;
            }
            None => {
                acp_engine::start_embedded_server(id, agent.clone(), mcp_registry.clone()).await
            }
        };
        match engine {
            Ok((url, handle, engine)) => {
                runner_impls.insert(
                    kind.clone(),
                    Arc::new(runner::AcpRunner::new(
                        kind.clone(),
                        url,
                        client.clone(),
                        engine.clone(),
                    )),
                );
                acp_engines.insert(kind, engine);
                server_handles.push(handle);
            }
            Err(e) => tracing::warn!(agent = %id, "ACP agent unavailable: {e}"),
        }
    }
    let runner_registry = Arc::new(runner::RunnerRegistry::new(default_runner, runner_impls));

    // Hand the engines started above to the supervisor, so a later edit to `acp.json` can
    // start, restart or drop an agent against the same runner slots rather than fighting
    // for them. Only engines that actually made it into the registry are adopted.
    let acp_supervisor = Arc::new(acp_engine::supervisor::AcpSupervisor::adopt(
        runner_registry.clone(),
        mcp_registry.clone(),
        client.clone(),
        acp_engines
            .into_iter()
            .filter(|(kind, _)| runner_registry.has(kind)),
    ));

    // The agent-manager MCP is attached to every runner.  MCP child processes
    // inherit this per-opman socket path and use it to reach the shared
    // registry, so cross-runner sends do not depend on which runner spawned
    // the child.
    let agent_manager_socket = mcp_agent_manager::spawn(runner_registry.clone())
        .context("Failed to start agent manager MCP")?;
    std::env::set_var(
        "OPMAN_AGENT_MANAGER_SOCKET",
        agent_manager_socket.to_string_lossy().as_ref(),
    );

    // Kill the server on Ctrl+C (even if the TUI hasn't reached cleanup)
    {
        let handles = server_handles.clone();
        ctrlc::set_handler(move || {
            for handle in &handles {
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
        web_port,
        &web_user,
        &web_pass,
        instance_name,
        backend.display_name(),
        &app,
        runner_registry.clone(),
        mcp_registry.clone(),
        acp_supervisor,
    )
    .await;

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
        tokio::signal::ctrl_c().await.ok();

        // Clean up MCP socket files
        if enable_any_mcp {
            for project in &app.projects {
                mcp::cleanup_socket(&project.path);
            }
        }

        for handle in &server_handles {
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
    for handle in &server_handles {
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
