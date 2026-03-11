//! Slash command handlers: /opman-projects, /opman-sessions, /opman-connect, etc.

use tracing::warn;

use super::super::api::post_message;
use super::super::types::SessionMeta;
use super::modals::{open_connect_project_modal, open_sessions_modal};

// ── Slash Command Handlers ─────────────────────────────────────────────

/// Outcome of a slash command: either fully handled, or needs triage.
pub enum SlashCommandOutcome {
    /// The command was fully handled (response posted via response_url or channel).
    Handled,
    /// The command requires AI triage. The caller (app.rs) should spawn triage
    /// with the provided `triage_text` and route the TriageResult back.
    NeedsTriage {
        /// The text to send to triage (may be rewritten, e.g. "connect to {name}").
        triage_text: String,
        /// When true, the triage result should be treated as connect-only (Action D).
        force_connect: bool,
        /// When true, the triage result should be treated as route (Action B).
        force_route: bool,
    },
}

/// Dispatch a slash command to the appropriate handler.
///
/// Returns `SlashCommandOutcome::Handled` if the command was fully handled,
/// or `SlashCommandOutcome::NeedsTriage` if triage needs to be spawned.
///
/// `projects` is `Vec<(name, path)>` (excluding slack-triage).
/// `sessions` is all sessions across projects (excluding slack-triage).
/// `trigger_id` is the short-lived ID from Slack for opening modals (3s expiry).
pub async fn handle_slash_command(
    command: &str,
    text: &str,
    channel: &str,
    response_url: &str,
    trigger_id: &str,
    projects: &[(String, String)],
    sessions: &[SessionMeta],
    bot_token: &str,
) -> SlashCommandOutcome {
    match command {
        "/opman-projects" => {
            handle_projects_slash(projects, channel, response_url, bot_token).await;
            SlashCommandOutcome::Handled
        }
        "/opman-sessions" => {
            let trimmed = text.trim();
            if trimmed.is_empty() && !trigger_id.is_empty() {
                // No args — open a modal picker for project selection.
                if let Err(e) = open_sessions_modal(trigger_id, channel, projects, bot_token).await
                {
                    warn!("Failed to open sessions modal: {}", e);
                    let client = reqwest::Client::new();
                    let _ = super::super::api::post_to_response_url(
                        &client,
                        response_url,
                        &format!(":x: Failed to open project picker: {}", e),
                        true,
                    )
                    .await;
                }
            } else {
                handle_sessions_slash(text, projects, sessions, channel, response_url, bot_token)
                    .await;
            }
            SlashCommandOutcome::Handled
        }
        "/opman-connect" => {
            let trimmed = text.trim();
            if trimmed.is_empty() && !trigger_id.is_empty() {
                // No args — open step-1 modal (project picker).
                if let Err(e) =
                    open_connect_project_modal(trigger_id, channel, projects, bot_token).await
                {
                    warn!("Failed to open connect project modal: {}", e);
                    let client = reqwest::Client::new();
                    let _ = super::super::api::post_to_response_url(
                        &client,
                        response_url,
                        &format!(":x: Failed to open project picker: {}", e),
                        true,
                    )
                    .await;
                }
                SlashCommandOutcome::Handled
            } else if trimmed.is_empty() {
                let client = reqwest::Client::new();
                let _ = super::super::api::post_to_response_url(
                    &client,
                    response_url,
                    ":warning: Usage: `/opman-connect <session or project name>`",
                    true,
                )
                .await;
                SlashCommandOutcome::Handled
            } else {
                // Rewrite as a connect request for triage.
                SlashCommandOutcome::NeedsTriage {
                    triage_text: format!("connect to {}", trimmed),
                    force_connect: true,
                    force_route: false,
                }
            }
        }
        "/opman-route" => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                let client = reqwest::Client::new();
                let _ = super::super::api::post_to_response_url(
                    &client,
                    response_url,
                    ":warning: Usage: `/opman-route <coding task description>`",
                    true,
                )
                .await;
                SlashCommandOutcome::Handled
            } else {
                SlashCommandOutcome::NeedsTriage {
                    triage_text: trimmed.to_string(),
                    force_connect: false,
                    force_route: true,
                }
            }
        }
        "/opman-ask" => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                let client = reqwest::Client::new();
                let _ = super::super::api::post_to_response_url(
                    &client,
                    response_url,
                    ":warning: Usage: `/opman-ask <question or task>`",
                    true,
                )
                .await;
                SlashCommandOutcome::Handled
            } else {
                // Full triage — no forced action.
                SlashCommandOutcome::NeedsTriage {
                    triage_text: trimmed.to_string(),
                    force_connect: false,
                    force_route: false,
                }
            }
        }
        _ => {
            let client = reqwest::Client::new();
            let _ = super::super::api::post_to_response_url(
                &client,
                response_url,
                &format!(":warning: Unknown command: `{}`", command),
                true,
            )
            .await;
            SlashCommandOutcome::Handled
        }
    }
}

/// Handle `/opman-projects` — list all configured projects.
async fn handle_projects_slash(
    projects: &[(String, String)],
    channel: &str,
    _response_url: &str,
    bot_token: &str,
) {
    let client = reqwest::Client::new();

    let project_list = if projects.is_empty() {
        "No projects configured.".to_string()
    } else {
        projects
            .iter()
            .map(|(name, path)| format!("• *{}*  `{}`", name, path))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let msg = format!(
        ":package: *Available Projects ({})* :\n{}",
        projects.len(),
        project_list,
    );

    // Post as a visible message in the channel (not ephemeral).
    let _ = post_message(&client, bot_token, channel, &msg, None).await;
}

/// Handle `/opman-sessions [project]` — list sessions, optionally filtered by project.
pub async fn handle_sessions_slash(
    text: &str,
    _projects: &[(String, String)],
    sessions: &[SessionMeta],
    channel: &str,
    response_url: &str,
    bot_token: &str,
) {
    let client = reqwest::Client::new();
    let filter = text.trim().to_lowercase();

    // Filter sessions: if a project name/prefix is given, only show matching sessions.
    let filtered: Vec<&SessionMeta> = sessions
        .iter()
        .filter(|s| s.parent_id.is_empty()) // skip subagents
        .filter(|s| {
            if filter.is_empty() {
                true
            } else {
                s.project_name.to_lowercase().contains(&filter)
            }
        })
        .collect();

    if filtered.is_empty() {
        let msg = if filter.is_empty() {
            ":information_source: No sessions found.".to_string()
        } else {
            format!(
                ":information_source: No sessions found matching project \"{}\".",
                filter
            )
        };
        let _ = super::super::api::post_to_response_url(&client, response_url, &msg, true).await;
        return;
    }

    // Sort by updated time descending.
    let mut sorted = filtered;
    sorted.sort_by(|a, b| b.updated.cmp(&a.updated));

    // Limit to 20 sessions for readability.
    let display: Vec<&SessionMeta> = sorted.into_iter().take(20).collect();

    let lines: Vec<String> = display
        .iter()
        .map(|s| {
            let short_id = &s.id[..8.min(s.id.len())];
            let title = if s.title.is_empty() {
                "(untitled)".to_string()
            } else {
                s.title.clone()
            };
            let updated = if s.updated > 0 {
                chrono::DateTime::from_timestamp(s.updated as i64, 0)
                    .map(|dt| dt.format("%m/%d %H:%M").to_string())
                    .unwrap_or_else(|| "?".to_string())
            } else {
                "?".to_string()
            };
            format!(
                "• `{}` *{}* — {} ({})",
                short_id, title, s.project_name, updated
            )
        })
        .collect();

    let header = if filter.is_empty() {
        format!(":card_file_box: *Sessions ({})* :", display.len())
    } else {
        format!(
            ":card_file_box: *Sessions matching \"{}\" ({})* :",
            filter,
            display.len()
        )
    };

    let msg = format!("{}\n{}", header, lines.join("\n"));
    let _ = post_message(&client, bot_token, channel, &msg, None).await;
}
