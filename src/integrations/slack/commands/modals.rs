//! Modal builders for slash commands.

use super::super::types::SessionMeta;

// ── Modal Builders ─────────────────────────────────────────────────────

/// Open a modal for `/opman-sessions` when invoked with no arguments.
///
/// Presents a dropdown of all project names. On submission, the selected
/// project is used to filter the sessions list.
pub(super) async fn open_sessions_modal(
    trigger_id: &str,
    channel: &str,
    projects: &[(String, String)],
    bot_token: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // Build options for the static_select dropdown.
    let mut options: Vec<serde_json::Value> = vec![serde_json::json!({
        "text": { "type": "plain_text", "text": "All projects" },
        "value": "__all__"
    })];
    for (name, _path) in projects {
        options.push(serde_json::json!({
            "text": { "type": "plain_text", "text": name },
            "value": name,
        }));
    }

    let view = serde_json::json!({
        "type": "modal",
        "callback_id": "opman_sessions_modal",
        "title": { "type": "plain_text", "text": "Filter Sessions" },
        "submit": { "type": "plain_text", "text": "Show Sessions" },
        "close": { "type": "plain_text", "text": "Cancel" },
        "private_metadata": channel,
        "blocks": [
            {
                "type": "input",
                "block_id": "project_select_block",
                "label": { "type": "plain_text", "text": "Select a project" },
                "element": {
                    "type": "static_select",
                    "action_id": "project_select_action",
                    "placeholder": { "type": "plain_text", "text": "Choose a project..." },
                    "initial_option": {
                        "text": { "type": "plain_text", "text": "All projects" },
                        "value": "__all__"
                    },
                    "options": options,
                }
            }
        ]
    });

    super::super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
    Ok(())
}

/// Open step-1 modal for `/opman-connect`: project picker.
///
/// On submission (`callback_id: "opman_connect_project_modal"`), the selected
/// project is used to filter sessions for step 2.
pub(super) async fn open_connect_project_modal(
    trigger_id: &str,
    channel: &str,
    projects: &[(String, String)],
    bot_token: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let options: Vec<serde_json::Value> = projects
        .iter()
        .map(|(name, _path)| {
            serde_json::json!({
                "text": { "type": "plain_text", "text": name },
                "value": name,
            })
        })
        .collect();

    if options.is_empty() {
        anyhow::bail!("No projects available.");
    }

    let view = serde_json::json!({
        "type": "modal",
        "callback_id": "opman_connect_project_modal",
        "title": { "type": "plain_text", "text": "Connect to Session" },
        "submit": { "type": "plain_text", "text": "Next" },
        "close": { "type": "plain_text", "text": "Cancel" },
        "private_metadata": channel,
        "blocks": [
            {
                "type": "input",
                "block_id": "project_select_block",
                "label": { "type": "plain_text", "text": "Select a project" },
                "element": {
                    "type": "static_select",
                    "action_id": "project_select_action",
                    "placeholder": { "type": "plain_text", "text": "Choose a project..." },
                    "options": options,
                }
            }
        ]
    });

    super::super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
    Ok(())
}

/// Open step-2 modal for `/opman-connect`: session picker + optional message.
///
/// `sessions` should already be filtered to the selected project.
/// On submission (`callback_id: "opman_connect_modal"`), triggers triage.
pub async fn open_connect_session_modal(
    trigger_id: &str,
    channel: &str,
    project_name: &str,
    sessions: &[SessionMeta],
    bot_token: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // Build session options for the dropdown (only root sessions for the project).
    let session_options: Vec<serde_json::Value> = sessions
        .iter()
        .filter(|s| s.parent_id.is_empty() && s.project_name == project_name)
        .map(|s| {
            let short_id = &s.id[..8.min(s.id.len())];
            let title = if s.title.is_empty() {
                "(untitled)".to_string()
            } else {
                // Truncate title to fit Slack's 75-char option text limit.
                let max_title_len = 60;
                if s.title.len() > max_title_len {
                    format!("{}...", &s.title[..max_title_len])
                } else {
                    s.title.clone()
                }
            };
            let label = format!("{} - {}", short_id, title);
            // Slack option text max is 75 chars.
            let label = if label.len() > 75 {
                format!("{}...", &label[..72])
            } else {
                label
            };
            serde_json::json!({
                "text": { "type": "plain_text", "text": label },
                "value": s.id,
            })
        })
        .collect();

    if session_options.is_empty() {
        anyhow::bail!("No sessions found for project '{}'.", project_name);
    }

    // Store channel in private_metadata so the final ViewSubmission handler
    // knows where to post the processing message and triage result.
    let view = serde_json::json!({
        "type": "modal",
        "callback_id": "opman_connect_modal",
        "title": { "type": "plain_text", "text": "Connect to Session" },
        "submit": { "type": "plain_text", "text": "Connect" },
        "close": { "type": "plain_text", "text": "Cancel" },
        "private_metadata": channel,
        "blocks": [
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Project:* {}", project_name),
                }
            },
            {
                "type": "input",
                "block_id": "session_select_block",
                "label": { "type": "plain_text", "text": "Select a session" },
                "element": {
                    "type": "static_select",
                    "action_id": "session_select_action",
                    "placeholder": { "type": "plain_text", "text": "Choose a session..." },
                    "options": session_options,
                }
            },
            {
                "type": "input",
                "block_id": "message_input_block",
                "label": { "type": "plain_text", "text": "Message (optional)" },
                "optional": true,
                "element": {
                    "type": "plain_text_input",
                    "action_id": "message_input_action",
                    "placeholder": { "type": "plain_text", "text": "Type a message to send to the session..." },
                    "multiline": false,
                }
            }
        ]
    });

    super::super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
    Ok(())
}
