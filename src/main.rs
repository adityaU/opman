mod api;
mod app;
mod background_tasks;
mod blockkit;
mod cli;
mod command_palette;
mod config;
mod event_input;
mod event_loop;
mod event_mouse;
mod input;
mod integrations;
mod mcp;
mod mcp_neovim;
mod mcp_skills;
mod mcp_time;
mod preflight;
mod mouse_handler;
mod process_health;
mod nvim_rpc;
mod pty;
mod server;
use integrations::slack;
mod setup;
mod sse;
mod theme;
mod theme_gen;
mod todo_db;
mod ui;
mod vim_mode;
mod util;
mod web;
mod which_key;

use std::io;

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
use crate::cli::{Cli, Commands, SkillsCommands};
use crate::config::Config;

async fn handle_skills(subcommand: SkillsCommands) -> anyhow::Result<()> {
    match subcommand {
        SkillsCommands::List => {
            let registry = crate::mcp_skills::load_skills().await?;
            for (name, skill) in registry {
                println!("{}: {}", name, skill.description);
            }
        }
        SkillsCommands::Create { name, description, content } => {
            let skill_dir = crate::mcp_skills::get_skills_dir().join(&name);
            std::fs::create_dir_all(&skill_dir)?;
            let skill_md = skill_dir.join("SKILL.md");
            let content_str = format!("---\nname: {}\ndescription: {}\n---\n{}", name, description, content);
            std::fs::write(&skill_md, content_str)?;
            println!("Skill '{}' created.", name);
        }
        SkillsCommands::Update { name, description, content } => {
            let skill_dir = crate::mcp_skills::get_skills_dir().join(&name);
            if !skill_dir.exists() {
                anyhow::bail!("Skill '{}' not found", name);
            }
            let skill_md = skill_dir.join("SKILL.md");
            let content_str = format!("---\nname: {}\ndescription: {}\n---\n{}", name, description, content);
            std::fs::write(&skill_md, content_str)?;
            println!("Skill '{}' updated.", name);
        }
        SkillsCommands::Delete { name } => {
            let skill_dir = crate::mcp_skills::get_skills_dir().join(&name);
            if !skill_dir.exists() {
                anyhow::bail!("Skill '{}' not found", name);
            }
            std::fs::remove_dir_all(&skill_dir)?;
            println!("Skill '{}' deleted.", name);
        }
        SkillsCommands::Show { name } => {
            let registry = crate::mcp_skills::load_skills().await?;
            if let Some(skill) = registry.get(&name) {
                println!("Name: {}", skill.name);
                println!("Description: {}", skill.description);
                println!("Content:\n{}", skill.content);
            } else {
                anyhow::bail!("Skill '{}' not found", name);
            }
        }
    }
    Ok(())
}

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
            return mcp::run_mcp_bridge(project_path).await.map_err(Into::into);
        }
        Some(Commands::McpTime) => {
            return mcp_time::run_mcp_time_bridge().await.map_err(Into::into);
        }
        Some(Commands::McpNvim { project_path }) => {
            return mcp_neovim::run_mcp_neovim_bridge(project_path)
                .await
                .map_err(Into::into);
        }
        Some(Commands::SlackManifest) => {
            return setup::handle_slack_manifest();
        }
        Some(Commands::Skills { subcommand }) => {
            return handle_skills(subcommand).await.map_err(Into::into);
        }
        None => {} // Default mode: run the TUI
    }

    // ── Validate CLI argument combinations ───────────────────────────
    if let Err(msg) = cli.validate() {
        eprintln!("error: {msg}");
        std::process::exit(2);
    }

    // ── Derive computed flags ────────────────────────────────────────
    let enable_web = cli.enable_web();
    let tunnel_mode = cli.tunnel_mode();

    let no_mcp = cli.no_mcp;
    let enable_terminal_mcp = !no_mcp && !cli.no_terminal_mcp;
    let enable_neovim_mcp = !no_mcp && !cli.no_neovim_mcp;
    let enable_time_mcp = !no_mcp && !cli.no_time_mcp;
    let enable_any_mcp = enable_terminal_mcp || enable_neovim_mcp || enable_time_mcp;

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
            1 => parts[0].to_string(),              // bare name, no dots
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

    // Spawn `opencode serve` on a free port before anything else
    let (base_url, server_handle) =
        server::spawn_opencode_server().context("Failed to start opencode serve")?;
    crate::app::init_base_url(base_url);

    // Kill the server on Ctrl+C (even if the TUI hasn't reached cleanup)
    {
        let handle = server_handle.clone();
        ctrlc::set_handler(move || {
            server::kill_server(&handle);
            std::process::exit(0);
        })
        .ok();
    }

    let config = Config::load().context("Failed to load config")?;

    // Deploy embedded opencode theme JSON files to ~/.config/opencode/themes/
    if let Err(e) = theme::deploy_embedded_themes() {
        tracing::warn!("Failed to deploy embedded themes: {}", e);
    }

    // Create background event channel and app state
    let (bg_tx, bg_rx) = mpsc::unbounded_channel::<BackgroundEvent>();
    let mut app = App::new(config, bg_tx.clone());

    // Generate theme files for PTY programs (neovim, zsh, gitui)
    if let Err(e) = theme_gen::write_theme_files(&app.theme) {
        tracing::warn!("Failed to write theme files: {}", e);
    }

    // Start web UI server (if enabled)
    let (web_actual_port, web_state_handle) =
        setup::setup_web_server(enable_web, web_port, &web_user, &web_pass, instance_name, &app).await;

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
                        let routines: Vec<crate::app::RoutineItem> =
                            defs.iter().map(crate::app::RoutineItem::from_definition).collect();
                        let _ = bg_tx2.send(crate::app::BackgroundEvent::RoutinesFetched { routines });
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
        println!(
            "opman web-only mode — web UI at http://localhost:{}",
            web_actual_port
        );
        if _tunnel_handle.is_some() {
            println!("  (also exposed via Cloudflare tunnel — see URL above)");
        }
        println!("Press Ctrl+C to stop.");
        tokio::signal::ctrl_c().await.ok();
        server::kill_server(&server_handle);
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
    setup::setup_initial_projects(
        &mut app,
        enable_any_mcp,
        enable_terminal_mcp,
        enable_neovim_mcp,
        enable_time_mcp,
    );

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
    server::kill_server(&server_handle);

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
