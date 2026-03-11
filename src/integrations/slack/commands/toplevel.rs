//! Top-level @ command helpers: list-projects, session routing, triage helpers.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::warn;

use super::super::api::{fetch_all_session_messages, post_message, send_user_message};
use super::super::state::SlackState;
use super::super::triage::triage_project_dir;
use super::super::types::SessionMeta;

// ── Top-Level @ Command Helpers ─────────────────────────────────────────

/// Handle the `@list-projects` top-level command.
/// Posts a list of all configured projects (excluding slack-triage) to the Slack
/// channel as a threaded reply.
pub async fn handle_list_projects_command(
    projects: &[(String, String)], // (name, path)
    channel: &str,
    ts: &str,
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
    let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
}

/// Build a specialized AI prompt that asks the triage AI to match a user query
/// against the list of all sessions and return the matching session ID.
fn build_session_match_prompt(sessions: &[SessionMeta], user_query: &str) -> String {
    let session_list: String = sessions
        .iter()
        .filter(|s| s.parent_id.is_empty()) // skip subagents
        .map(|s| {
            let short_id = &s.id[..8.min(s.id.len())];
            let title = if s.title.is_empty() {
                "(untitled)".to_string()
            } else {
                s.title.clone()
            };
            let updated = if s.updated > 0 {
                chrono::DateTime::from_timestamp(s.updated as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "unknown".to_string()
            };
            format!(
                "  - ID: {} | Title: \"{}\" | Project: \"{}\" | Updated: {}",
                short_id, title, s.project_name, updated
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are helping route a Slack command to a specific coding session.

The user typed `@session {user_query} <message>`.
Your job is to:
1. Determine which session they are referring to by "{user_query}"
2. Rewrite the message portion to contain ONLY the actual task or question — strip ALL routing metadata

Available sessions:
{session_list}

Respond with EXACTLY this JSON format (no markdown, no explanation):
{{"session_id": "<full session id>", "session_title": "<title>", "project_name": "<project>", "rewritten_query": "<the user's actual task/question with ALL routing metadata removed>", "confidence": <0.0-1.0>}}

If you cannot determine the session with reasonable confidence (>0.5), respond:
{{"session_id": null, "confidence": 0.0, "error": "Could not determine which session you mean.", "candidates": ["<id_prefix>: <title> (project)", ...]}}

Rules:
- Match session titles loosely (abbreviations, partial names, keywords are OK).
- Also match by session ID prefix (e.g. "abc123" should match a session whose ID starts with "abc123").
- Prefer more recently updated sessions if multiple match equally well.
- The "candidates" field in error responses should list the top 5 closest matches.
- CRITICAL — rewritten_query rules:
  - The message the user wants to send follows the session identifier. Extract it and clean it.
  - Remove ALL session names, session IDs, project names, project paths, and routing instructions from the message.
  - Strip phrases like "in session X", "to session Y", "in the Z project", "@session", etc.
  - The rewritten_query must read as a clean, standalone message — as if the user typed it directly to a coding assistant.
  - Keep ONLY the substantive task, question, or instruction.
  - If there is no message beyond the session identifier, set rewritten_query to null.
"#,
        user_query = user_query,
        session_list = session_list,
    )
}
/// Helper: get the triage session ID by fetching sessions from the triage project.
/// Returns the session ID, or an empty string if none found.
async fn get_triage_session_id(
    client: &reqwest::Client,
    base_url: &str,
    triage_dir: &str,
) -> String {
    let sessions_url = format!("{}/session", base_url);
    let sessions_resp = client
        .get(&sessions_url)
        .header("x-opencode-directory", triage_dir)
        .header("Accept", "application/json")
        .send()
        .await;

    match sessions_resp {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
                arr.iter().collect()
            } else if let Some(obj) = body.as_object() {
                obj.values().collect()
            } else {
                vec![]
            };
            let triage_session = items.iter().find(|s| {
                let dir = s.get("directory").and_then(|v| v.as_str()).unwrap_or("");
                dir == triage_dir
            });
            triage_session
                .and_then(|s| s.get("id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string()
        }
        Err(e) => {
            warn!("@ command: failed to fetch triage sessions: {}", e);
            String::new()
        }
    }
}

/// Helper: send a prompt to the triage AI and wait for its JSON response.
/// Returns the raw AI response text, or an error string.
async fn send_triage_and_wait(
    client: &reqwest::Client,
    base_url: &str,
    triage_dir: &str,
    session_id: &str,
    prompt: &str,
) -> Result<String, String> {
    send_user_message(client, base_url, triage_dir, session_id, prompt)
        .await
        .map_err(|e| format!("Failed to send triage prompt: {}", e))?;

    // Wait for the AI to respond.
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let messages = fetch_all_session_messages(client, base_url, triage_dir, session_id)
        .await
        .map_err(|e| format!("Failed to fetch triage response: {}", e))?;

    let ai_response = messages
        .iter()
        .rev()
        .find(|(role, _)| role == "assistant")
        .map(|(_, text)| text.clone())
        .unwrap_or_default();

    if ai_response.is_empty() {
        Err("Triage AI did not respond.".to_string())
    } else {
        Ok(ai_response)
    }
}

/// Handle `@session <fuzzy session name> <message>` using AI triage for session matching.
///
/// Sends a specialized prompt to the triage AI to fuzzy-match the session,
/// then routes the message to the matched session.
pub async fn handle_session_command(
    session_query: &str,
    message_text: &str,
    all_sessions: &[SessionMeta],
    channel: &str,
    ts: &str,
    bot_token: &str,
    base_url: &str,
    buffer_secs: u64,
    slack_state: Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    // Get the triage project directory.
    let triage_dir = match triage_project_dir() {
        Ok(d) => d.to_string_lossy().to_string(),
        Err(e) => {
            let msg = format!(
                ":x: Internal error — could not locate triage project: {}",
                e
            );
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    // Get triage session.
    let triage_session_id = get_triage_session_id(&client, base_url, &triage_dir).await;
    if triage_session_id.is_empty() {
        let msg = ":x: No triage session available. Please ensure the Slack triage project has at least one session.";
        let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // Build specialized session-matching prompt and send to triage AI.
    let prompt = build_session_match_prompt(all_sessions, session_query);
    let ai_response =
        match send_triage_and_wait(&client, base_url, &triage_dir, &triage_session_id, &prompt)
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!(":x: Triage failed: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
                return;
            }
        };

    // Parse the AI response as JSON.
    let parsed: serde_json::Value = match serde_json::from_str(&ai_response) {
        Ok(v) => v,
        Err(_) => {
            // Try to extract JSON from markdown code fences.
            let stripped = ai_response
                .trim()
                .strip_prefix("```json")
                .or_else(|| ai_response.trim().strip_prefix("```"))
                .unwrap_or(&ai_response)
                .trim()
                .strip_suffix("```")
                .unwrap_or(&ai_response)
                .trim();
            match serde_json::from_str(stripped) {
                Ok(v) => v,
                Err(_) => {
                    let msg = ":warning: Could not parse AI response for session matching. Use `@list-sessions <project>` to find session names.";
                    let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
                    return;
                }
            }
        }
    };

    // Check for error / low confidence, match session, route, and connect relay.
    super::session_route::handle_session_command_cont(
        &client,
        &parsed,
        all_sessions,
        channel,
        ts,
        bot_token,
        base_url,
        buffer_secs,
        message_text,
        slack_state,
    )
    .await;
}
