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
mod doctor;
mod event_input;
mod event_loop;
mod event_mouse;
mod input;
mod integrations;
mod ipc;
mod logging;
mod loopback;
mod lsp;
mod mcp;
mod mcp_agent_manager;
mod mcp_ask;
mod mcp_browser;
mod mcp_code;
mod mcp_kanban;
mod mcp_neovim;
mod mcp_oauth;
mod mcp_probe;
mod mcp_proxy;
mod mcp_registry;
mod mcp_skills;
mod mcp_time;
mod mcp_tools;
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
mod self_update;
mod server;
mod session_disk;
use integrations::slack;
mod setup;
mod sse;
mod startup;
mod theme;
mod theme_gen;
mod todo_db;
mod ui;
mod usage;
mod util;
mod vim_mode;
mod web;
mod which_key;

use crate::cli::{Cli, Commands};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Parse CLI arguments, then start file logging ─────────────────
    let cli = Cli::parse();
    let log_path = logging::init(cli.debug)?;
    if cli.debug {
        tracing::debug!(log = %log_path.display(), "debug logging on");
    }

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
        Some(Commands::McpCode { project_path }) => {
            return mcp_code::run_mcp_code_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
            .await;
        }
        Some(Commands::McpAsk { project_path }) => {
            return mcp_ask::run_mcp_ask_bridge(project_path.unwrap_or_else(|| PathBuf::from(".")))
                .await;
        }
        Some(Commands::McpTools { project_path }) => {
            return mcp_tools::run_mcp_tools_bridge(
                project_path.unwrap_or_else(|| PathBuf::from(".")),
            )
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
        Some(Commands::Update { version }) => {
            return self_update::update(version).await;
        }
        Some(Commands::Downgrade { version }) => {
            return self_update::downgrade(version).await;
        }
        Some(Commands::Doctor { json, agent }) => {
            return doctor::run(json, agent).await;
        }
        None => {} // Default mode: run the TUI
    }

    startup::run(cli).await
}
