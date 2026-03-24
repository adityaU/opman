use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{PollWatcher, RecursiveMode, Watcher};
use tracing::info;

use crate::app::{self, App};
use crate::background_tasks::spawn_activate_project;
use crate::integrations::slack;
use crate::mcp;
use crate::sse;
use crate::web;

/// Handle the `slack-manifest` subcommand.
pub(crate) fn handle_slack_manifest() -> Result<()> {
    const MANIFEST: &str = include_str!("../slack-app-manifest.yaml");
    // Try to copy to clipboard (macOS: pbcopy, Linux: xclip/xsel)
    let copied = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(MANIFEST.as_bytes())?;
            }
            child.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
        || std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(MANIFEST.as_bytes())?;
                }
                child.wait()
            })
            .map(|s| s.success())
            .unwrap_or(false);

    println!("{}", MANIFEST);
    if copied {
        eprintln!("\n✓ Manifest copied to clipboard");
    }
    eprintln!("\nTo create your Slack app:");
    eprintln!("  1. Go to https://api.slack.com/apps → Create New App → From a manifest");
    eprintln!("  2. Select your workspace and paste the manifest above");
    eprintln!(
        "  3. After creating, generate an App-Level Token with `connections:write` scope"
    );
    eprintln!("  4. Install the app to your workspace");
    Ok(())
}

/// Setup and start the web server if enabled.
pub(crate) async fn setup_web_server(
    enable_web: bool,
    web_port: Option<u16>,
    web_user: &str,
    web_pass: &str,
    instance_name: Option<String>,
    app: &App,
) -> (u16, Option<web::WebStateHandle>) {
    if enable_web {
        let (actual_port, wsh) = web::start_web_server(
            web::WebConfig {
                port: web_port,
                username: web_user.to_string(),
                password: web_pass.to_string(),
                instance_name,
            },
            app.nvim_registry.clone(),
        ).await;
        info!("Web UI available at http://localhost:{}", actual_port);
        // Set the initial theme so web clients can fetch it immediately
        let initial_theme = web::WebThemePair::from_active_theme();
        let wsh_clone = wsh.clone();
        tokio::spawn(async move {
            wsh_clone.set_theme(initial_theme).await;
        });
        (actual_port, Some(wsh))
    } else {
        (0, None)
    }
}

/// Setup the KV file watcher for theme reloading.
pub(crate) fn setup_kv_watcher() -> Result<(std::sync::mpsc::Receiver<notify::Event>, PollWatcher)>
{
    let kv_path = {
        let state_dir = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/state")
            });
        state_dir.join("opencode/kv.json")
    };

    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();
    let poll_config = notify::Config::default().with_poll_interval(Duration::from_millis(500));
    let mut watcher = PollWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = watcher_tx.send(event);
            }
        },
        poll_config,
    )
    .context("Failed to create file watcher")?;

    if kv_path.exists() {
        watcher
            .watch(&kv_path, RecursiveMode::NonRecursive)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to watch KV file: {}", e);
            });
    } else if let Some(kv_dir) = kv_path.parent() {
        if kv_dir.exists() {
            watcher
                .watch(kv_dir, RecursiveMode::NonRecursive)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to watch KV directory: {}", e);
                });
        }
    }

    Ok((watcher_rx, watcher))
}

/// Kick off initial data loading for all projects.
pub(crate) fn setup_initial_projects(
    app: &mut App,
    enable_any_mcp: bool,
    enable_terminal_mcp: bool,
    enable_neovim_mcp: bool,
    enable_time_mcp: bool,
) {
    if app.projects.is_empty() {
        return;
    }

    // Fetch sessions for ALL projects immediately
    crate::background_tasks::spawn_session_fetch(&app.bg_tx, &app.projects);

    // Start SSE listeners and session pollers for ALL projects
    for i in 0..app.projects.len() {
        let dir = app.projects[i].path.to_string_lossy().to_string();
        sse::spawn_sse_listener(&app.bg_tx, i, dir.clone());
        sse::spawn_session_poller(&app.bg_tx, i, dir.clone());
        sse::spawn_provider_fetcher(&app.bg_tx, i, dir);
    }

    // Spawn MCP socket servers and write opencode.json for each project
    if enable_any_mcp {
        for i in 0..app.projects.len() {
            let project_path = app.projects[i].path.clone();
            if enable_terminal_mcp || enable_neovim_mcp {
                mcp::spawn_socket_server(
                    &project_path,
                    app.bg_tx.clone(),
                    i,
                    app.nvim_registry.clone(),
                    app.last_mcp_activity_ms.clone(),
                );
            }
            if let Err(e) = mcp::write_opencode_json(
                &project_path,
                enable_terminal_mcp,
                enable_neovim_mcp,
                enable_time_mcp,
            ) {
                tracing::warn!(
                    "Failed to write opencode.json for {}: {}",
                    project_path.display(),
                    e
                );
            }
        }
    }

    // When neovim MCP is enabled, store flag on App (disables follow-edits)
    if enable_neovim_mcp {
        app.neovim_mcp_enabled = true;
    }

    // Auto-activate project 0 by spawning PTY directly
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let content_area = ratatui::layout::Rect::new(0, 0, cols, rows.saturating_sub(1));
    app.layout.compute_rects(content_area);
    let (inner_cols, inner_rows) = app
        .layout
        .panel_rect(crate::ui::layout_manager::PanelId::TerminalPane)
        .map(|r| (r.width, r.height))
        .unwrap_or((cols.saturating_sub(32), rows.saturating_sub(2)));
    let path = app.projects[0].path.clone();
    let theme_envs = app.theme.pty_env_vars();
    spawn_activate_project(&app.bg_tx, 0, path, inner_rows, inner_cols, theme_envs);

    // Auto-start neovim PTY for all projects when neovim MCP is enabled
    if enable_neovim_mcp {
        let saved = app.active_project;
        for i in 0..app.projects.len() {
            app.active_project = i;
            app.ensure_neovim_pty();
        }
        app.active_project = saved;
    }
}

/// Start Slack integration if enabled and credentials are available.
pub(crate) fn setup_slack(app: &mut App) {
    if !app.config.settings.slack.enabled {
        return;
    }

    match slack::SlackAuth::load() {
        Ok(Some(auth)) => {
            if !auth.app_token.is_empty() && !auth.bot_token.is_empty() {
                info!("Slack integration enabled, starting Socket Mode...");
                let slack_state =
                    std::sync::Arc::new(tokio::sync::Mutex::new(slack::SlackState::new()));
                app.slack_state = Some(slack_state.clone());
                app.slack_auth = Some(auth.clone());

                // Spawn Socket Mode listener.
                let bg_tx_slack = app.bg_tx.clone();
                let auth_clone = auth.clone();
                tokio::spawn(async move {
                    let (slack_event_tx, mut slack_event_rx) =
                        tokio::sync::mpsc::unbounded_channel();

                    // Spawn the WebSocket listener.
                    tokio::spawn(slack::spawn_socket_mode(auth_clone, slack_event_tx));

                    // Forward Slack events to the main app background event channel.
                    while let Some(event) = slack_event_rx.recv().await {
                        let _ = bg_tx_slack.send(app::BackgroundEvent::SlackEvent(event));
                    }
                });

                // Spawn response batcher.
                let batch_secs = app.config.settings.slack.response_batch_secs;
                let auth_batch = auth;
                let state_batch = slack_state;
                tokio::spawn(async move {
                    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
                    slack::spawn_response_batcher(
                        auth_batch,
                        state_batch,
                        batch_secs,
                        event_tx,
                    )
                    .await;
                });
            } else {
                info!("Slack enabled but missing tokens in slack_auth.yaml (need bot_token + app_token)");
            }
        }
        Ok(None) => {
            info!("Slack enabled but no slack_auth.yaml found. Run OAuth to connect.");
        }
        Err(e) => {
            tracing::warn!("Failed to load Slack auth: {}", e);
        }
    }
}
