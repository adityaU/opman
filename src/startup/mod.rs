use std::io;

use anyhow::{Context, Result};
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
use crate::cli::Cli;
use crate::config::Config;

use crate::{event_loop, mcp, mcp_agent_manager, preflight, server, setup, theme_gen, web};
mod runners;
pub(crate) async fn run(cli: Cli) -> Result<()> {
    // ── Validate CLI argument combinations ───────────────────────────
    if let Err(msg) = cli.validate() {
        eprintln!("error: {msg}");
        std::process::exit(2);
    }

    // ── Derive computed flags ────────────────────────────────────────
    let backend = cli.resolved_backend();
    let enable_web = cli.enable_web();
    let tunnel_mode = cli.tunnel_mode();

    cli.warn_deprecated_mcp_flags();

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

    let runners::Runners {
        registry: runner_registry,
        mcp: mcp_registry,
        supervisor: acp_supervisor,
        context: lazy_ctx,
    } = runners::boot(backend).await?;
    // Whether the terminal or Neovim bridge (both switched on in Settings) left sockets to clean up.
    let socket_mcp = {
        let servers = mcp_registry.current();
        servers.has("terminal") || servers.has("neovim")
    };
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
        setup::setup_initial_projects(&mut app, &mcp_registry.current(), true);

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
        if socket_mcp {
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
    setup::setup_initial_projects(&mut app, &mcp_registry.current(), false);

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
    if socket_mcp {
        for project in &app.projects {
            mcp::cleanup_socket(&project.path);
        }
    }

    info!("opman shut down");

    result
}
