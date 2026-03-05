//! Top-level @ commands and thread slash commands for Slack.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use super::api::{
    fetch_all_session_messages, post_message, post_message_with_blocks, send_user_message,
    start_stream, stop_stream, update_message_blocks,
};
use super::auth::SlackSessionMap;
use super::relay::spawn_session_relay_watcher;
use super::state::SlackState;
use super::triage::triage_project_dir;
use super::types::{SessionMeta, SlackLogLevel};

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

/// Handle `@list-sessions <fuzzy project name>` using AI triage for project matching.
///
/// Sends a specialized prompt to the triage AI to fuzzy-match the project,
/// then fetches and formats the last 5 sessions for that project.

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

    // Check for error / low confidence.
    if let Some(err) = parsed
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let mut msg = format!(":warning: {}", err);
        if let Some(candidates) = parsed.get("candidates").and_then(|v| v.as_array()) {
            let list: Vec<String> = candidates
                .iter()
                .filter_map(|c| c.as_str().map(|s| format!("• {}", s)))
                .collect();
            if !list.is_empty() {
                msg.push_str(&format!(
                    "\n\nDid you mean one of these?\n{}",
                    list.join("\n")
                ));
            }
        }
        let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
        return;
    }

    // Extract matched session ID. The AI returns the full session ID.
    let matched_session_id_raw = parsed
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    if matched_session_id_raw.is_empty() {
        let msg = ":warning: AI could not match a session. Use `@list-sessions <project>` to find session names.";
        let _ = post_message(&client, bot_token, channel, msg, Some(ts)).await;
        return;
    }

    // The AI might return a prefix; find the full session by prefix match.
    let matched_meta = all_sessions
        .iter()
        .find(|s| s.id == matched_session_id_raw || s.id.starts_with(matched_session_id_raw));

    let matched_meta = match matched_meta {
        Some(m) => m,
        None => {
            let msg = format!(
                ":warning: AI matched session ID `{}` but it was not found in the session list.",
                matched_session_id_raw
            );
            let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
            return;
        }
    };

    let session_id = matched_meta.id.clone();
    let session_name = if matched_meta.title.is_empty() {
        format!("Session {}", &session_id[..8.min(session_id.len())])
    } else {
        matched_meta.title.clone()
    };
    let project_name = matched_meta.project_name.clone();
    let project_dir = matched_meta.project_dir.clone();
    let pidx = matched_meta.project_idx;

    {
        let mut s = slack_state.lock().await;
        // Detach any existing relay for this session on a *different* thread.
        if let Some((old_ts, _)) = s.detach_relay_by_session(&session_id) {
            info!(
                "@ command: detached relay for session {} from old thread {}",
                &session_id[..8.min(session_id.len())],
                old_ts
            );
        }
        // One relay per thread: detach old session before attaching new.
        if let Some(old_sid) = s.detach_relay(ts) {
            info!(
                "@ command: detached previous relay (session {}) from thread {}",
                &old_sid[..8.min(old_sid.len())],
                ts
            );
        }
    }

    // Record thread→session mapping and mark this as the active relay.
    {
        let mut s = slack_state.lock().await;
        s.thread_sessions
            .insert(ts.to_string(), (pidx, session_id.clone()));
        s.session_threads
            .insert(session_id.clone(), (channel.to_string(), ts.to_string()));
        s.active_relay.insert(ts.to_string(), session_id.clone());
        s.metrics.messages_routed += 1;
        s.metrics.last_routed_at = Some(std::time::Instant::now());
        s.log(
            SlackLogLevel::Info,
            format!(
                "@ command routed to project \"{}\" session {}",
                project_name,
                &session_id[..8.min(session_id.len())]
            ),
        );
    }

    // Record current message offset so relay only shows new messages.
    // Use the same fetch function as the relay watcher for consistent counts.
    match super::tools::fetch_session_messages_with_tools(
        &client,
        base_url,
        &project_dir,
        &session_id,
    )
    .await
    {
        Ok(msgs) => {
            let mut s = slack_state.lock().await;
            s.session_msg_offset.insert(session_id.clone(), msgs.len());
            debug!(
                "@ command: recorded msg offset {} for session {}",
                msgs.len(),
                session_id
            );
        }
        Err(e) => {
            warn!(
                "@ command: failed to fetch msg offset for session {}: {}",
                session_id, e
            );
        }
    }

    // If a message was provided, send it to the session.
    // If no message was provided, this is a relay-only attach (no message sent).
    let has_message = !message_text.is_empty();
    if has_message {
        // Use the AI's rewritten query if available, otherwise fall back to raw message.
        let final_message = parsed
            .get("rewritten_query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .unwrap_or(message_text);

        // Send the user message to the session.
        match send_user_message(&client, base_url, &project_dir, &session_id, final_message).await {
            Ok(()) => {
                info!("@ command: user message sent to session {}", session_id);
                let ack = format!(
                    "relayed to project: {}, session: {}",
                    project_name, session_name
                );
                let _ = post_message(&client, bot_token, channel, &ack, Some(ts)).await;
            }
            Err(e) => {
                error!("@ command: failed to send message to session: {}", e);
                let msg = format!(":x: Failed to send message: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(ts)).await;
                return;
            }
        }
    } else {
        // Relay-only: acknowledge the attachment without sending a message.
        let ack = format!(
            ":link: attached to project: {}, session: {} (relay only, no message sent)",
            project_name, session_name
        );
        let _ = post_message(&client, bot_token, channel, &ack, Some(ts)).await;
    }

    // Spawn relay watcher (always — we detached the old one above so this is fresh).
    let already_watching = {
        let s = slack_state.lock().await;
        s.relay_abort_handles.contains_key(&session_id)
    };
    if !already_watching {
        let handle = spawn_session_relay_watcher(
            session_id.clone(),
            project_dir,
            channel.to_string(),
            ts.to_string(),
            bot_token.to_string(),
            base_url.to_string(),
            buffer_secs,
            slack_state.clone(),
        );
        let mut s = slack_state.lock().await;
        s.relay_abort_handles
            .insert(session_id.clone(), handle.abort_handle());

        // Persist session map to disk.
        let map = SlackSessionMap {
            session_threads: s.session_threads.clone(),
            thread_sessions: s.thread_sessions.clone(),
            msg_offsets: s.session_msg_offset.clone(),
        };
        if let Err(e) = map.save() {
            warn!("@ command: failed to persist session map: {}", e);
        }
    }
}

// ── Slack Thread Slash Commands ─────────────────────────────────────────

/// Handle a slash command sent in a Slack thread.
///
/// Returns `true` if the text was recognized as a command (and handled),
/// `false` if it should be treated as a normal thread reply.
pub async fn handle_thread_slash_command(
    text: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
) -> bool {
    let trimmed = text.trim();
    let (cmd, args) = match trimmed.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (trimmed, ""),
    };

    match cmd {
        "@stop" => {
            do_stop_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                slack_state,
            )
            .await;
            true
        }
        "@watcher" => {
            do_watcher_command(
                channel,
                thread_ts,
                session_id,
                project_idx,
                project_dir,
                bot_token,
                base_url,
                slack_state,
                watcher_inserted,
                watcher_removed,
                args,
            )
            .await;
            true
        }
        "@compact" | "@summarize" => {
            do_compact_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
        "@status" => {
            do_status_command(
                channel,
                thread_ts,
                session_id,
                project_idx,
                project_dir,
                bot_token,
                base_url,
                slack_state,
            )
            .await;
            true
        }
        "@todos" => {
            do_todos_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
        "@detach" => {
            do_detach_command(channel, thread_ts, session_id, bot_token, slack_state).await;
            true
        }
        "@messages" => {
            do_messages_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                args,
            )
            .await;
            true
        }
        "@undo" => {
            do_command_api(
                "undo",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":leftwards_arrow_with_hook: Undo triggered.",
                ":x: Undo failed",
            )
            .await;
            true
        }
        "@redo" => {
            do_command_api(
                "redo",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":arrow_right_hook: Redo triggered.",
                ":x: Redo failed",
            )
            .await;
            true
        }
        "@model" | "@models" => {
            do_model_command(
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                args,
            )
            .await;
            true
        }
        "@export" => {
            do_command_api(
                "export",
                "",
                None,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
                ":outbox_tray: Session exported.",
                ":x: Export failed",
            )
            .await;
            true
        }
        "@test_blockkit" => {
            do_test_blockkit_command(channel, thread_ts, bot_token).await;
            true
        }
        "@help" => {
            do_help_command(channel, thread_ts, bot_token).await;
            true
        }
        _ => {
            // Unrecognized @ command — try passthrough via command API,
            // fall back to help if the server rejects it.
            let oc_cmd = cmd.strip_prefix('@').unwrap_or(cmd);
            do_passthrough_command(
                oc_cmd,
                args,
                channel,
                thread_ts,
                session_id,
                project_dir,
                bot_token,
                base_url,
            )
            .await;
            true
        }
    }
}

/// `@stop` — Cancel the running OpenCode session (abort LLM generation and tool
/// execution).  The relay watcher and stream are left intact so that the final
/// state is still delivered to Slack once the session becomes idle.
async fn do_stop_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    _slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    // Abort the OpenCode session via server API.
    // This calls POST /session/{id}/abort which cancels any running LLM
    // generation and tool execution on the server side.
    let api = crate::api::ApiClient::new();
    let msg = match api.abort_session(base_url, project_dir, session_id).await {
        Ok(()) => {
            tracing::info!(
                "Slack @stop: aborted session {} via API",
                &session_id[..8.min(session_id.len())]
            );
            ":octagonal_sign: Session interrupted. The relay watcher will deliver any remaining output.".to_string()
        }
        Err(e) => {
            tracing::warn!(
                "Slack @stop: failed to abort session {}: {}",
                &session_id[..8.min(session_id.len())],
                e
            );
            format!(":warning: Failed to stop session: {}", e)
        }
    };
    let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
}

/// `@watcher` — Start or stop a continuation + hang protection watcher for this session.
///
/// The actual `WatcherConfig` insertion/removal happens inline in `app.rs`
/// (synchronous context with `&mut self` access).  The `watcher_inserted` and
/// `watcher_removed` flags tell us what happened so we can post the right
/// confirmation to Slack.  `args` is the subcommand text after `@watcher`
/// (e.g. "stop", "off", "remove", or "").
async fn do_watcher_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    _project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
    watcher_inserted: bool,
    watcher_removed: bool,
    args: &str,
) {
    let client = reqwest::Client::new();

    let is_stop = matches!(args, "stop" | "off" | "remove");

    // --- Watcher removal case (`@watcher stop` / `off` / `remove`) ---
    if watcher_removed {
        let msg = ":no_entry_sign: Watcher removed for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // User asked to stop but no watcher was active.
    if is_stop {
        let msg = ":information_source: No watcher is active for this thread.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // --- Watcher insertion case (`@watcher` with no subcommand) ---
    if watcher_inserted {
        let msg =
            ":eyes: Watcher enabled for this thread.\n• Idle timeout: 15s\n• Hang detection: 180s";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
    } else {
        let msg = ":warning: Could not enable watcher — failed to acquire lock on session watchers. Please try again.";
        let _ = post_message(&client, bot_token, channel, msg, Some(thread_ts)).await;
        return;
    }

    // Also ensure the relay watcher is running.
    {
        let s = slack_state.lock().await;
        if !s.relay_abort_handles.contains_key(session_id) {
            drop(s);
            // Need to re-spawn relay watcher.
            let handle = spawn_session_relay_watcher(
                session_id.to_string(),
                project_dir.to_string(),
                channel.to_string(),
                thread_ts.to_string(),
                bot_token.to_string(),
                base_url.to_string(),
                3,
                slack_state.clone(),
            );
            let mut s = slack_state.lock().await;
            s.relay_abort_handles
                .insert(session_id.to_string(), handle.abort_handle());
            tracing::info!(
                "Slack @watcher: re-spawned relay watcher for session {}",
                &session_id[..8.min(session_id.len())]
            );
        }
    }
}

/// `@compact` / `@summarize` — Compact/summarize the current session via the
/// OpenCode command API (`POST /session/:id/command { command: "compact" }`).
async fn do_compact_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    do_command_api(
        "compact",
        "",
        None,
        channel,
        thread_ts,
        session_id,
        project_dir,
        bot_token,
        base_url,
        ":recycle: Compaction triggered — session will summarize and continue.",
        ":x: Compaction failed",
    )
    .await;
}

/// `@test_blockkit` — Run a battery of tests to determine which Block Kit /
/// markdown approaches work with Slack's streaming API.
///
/// Runs 4 tests sequentially, each starting a short-lived stream:
///   A. Raw markdown table (no code-fence wrapping) via `markdown_text` chunk
///   B. `markdown` chunk type (instead of `markdown_text`)
///   C. Stream → `chat.update` with Block Kit `markdown` block
///   D. `chat.postMessage` with `markdown` block
async fn do_test_blockkit_command(channel: &str, thread_ts: &str, bot_token: &str) {
    let client = reqwest::Client::new();

    let _ = post_message(
        &client,
        bot_token,
        channel,
        ":test_tube: Starting Block Kit test battery…",
        Some(thread_ts),
    )
    .await;

    // Sample markdown with a table, code block, bold, italic, link, list
    let sample_md = concat!(
        "## Test Results\n\n",
        "Here is a **bold** word, an *italic* word, and `inline code`.\n\n",
        "| Feature | Status | Notes |\n",
        "|---------|--------|-------|\n",
        "| Tables  | :white_check_mark: | Native rendering |\n",
        "| Code    | :white_check_mark: | Fenced blocks |\n",
        "| Links   | :white_check_mark: | [Slack](https://slack.com) |\n\n",
        "```rust\nfn main() {\n    println!(\"Hello, Block Kit!\");\n}\n```\n\n",
        "- [ ] Todo item one\n",
        "- [x] Todo item done\n",
        "- [ ] Todo item three\n",
    );

    // ── Test A: Raw markdown table via streaming `markdown_text` ────────
    {
        let label =
            "**Test A**: Raw markdown table in `markdown_text` chunk (no code-fence wrapping)";
        let text = format!("{}\n\n{}", label, sample_md);

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            Some(&text),
            None,
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test A stream completed (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test A failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test B: `markdown` chunk type (instead of `markdown_text`) ──────
    {
        let label = "**Test B**: `markdown` chunk type in streaming API";
        let text = format!("{}\n\n{}", label, sample_md);

        // Build a chunk with type "markdown" instead of "markdown_text"
        let md_chunk = serde_json::json!({
            "type": "markdown",
            "text": text,
        });

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            None,
            Some(&[md_chunk]),
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test B stream completed (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test B failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test C: Stream then chat.update with Block Kit `markdown` block ─
    {
        let label = "**Test C**: Stream → chat.update with `markdown` block";

        match start_stream(
            &client,
            bot_token,
            channel,
            thread_ts,
            Some(label),
            None,
            None,
        )
        .await
        {
            Ok(ts) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let _ = stop_stream(&client, bot_token, channel, &ts).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Now update the stopped stream message with a markdown block
                let md_block = serde_json::json!({
                    "type": "markdown",
                    "text": format!("{}\n\n{}", label, sample_md),
                });
                let result = update_message_blocks(
                    &client,
                    bot_token,
                    channel,
                    &ts,
                    "Block Kit test C fallback",
                    &[md_block],
                    None,
                )
                .await;
                match result {
                    Ok(()) => {
                        let _ = post_message(
                            &client,
                            bot_token,
                            channel,
                            ":white_check_mark: Test C update completed (check rendering above)",
                            Some(thread_ts),
                        )
                        .await;
                    }
                    Err(e) => {
                        let _ = post_message(
                            &client,
                            bot_token,
                            channel,
                            &format!(":x: Test C update failed: {}", e),
                            Some(thread_ts),
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test C stream failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test D: chat.postMessage with `markdown` block ──────────────────
    {
        let label = "**Test D**: `chat.postMessage` with `markdown` block";
        let md_block = serde_json::json!({
            "type": "markdown",
            "text": format!("{}\n\n{}", label, sample_md),
        });

        match post_message_with_blocks(
            &client,
            bot_token,
            channel,
            "Block Kit test D fallback",
            &[md_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test D posted (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test D failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    // ── Test E: chat.postMessage with rich_text block (table) ───────────
    {
        let rich_text_block = serde_json::json!({
            "type": "rich_text",
            "elements": [
                {
                    "type": "rich_text_section",
                    "elements": [
                        {
                            "type": "text",
                            "text": "Test E: ",
                            "style": { "bold": true }
                        },
                        {
                            "type": "text",
                            "text": "rich_text block with styled text, code, and quote"
                        }
                    ]
                },
                {
                    "type": "rich_text_preformatted",
                    "elements": [
                        {
                            "type": "text",
                            "text": "fn main() {\n    println!(\"Hello from rich_text!\");\n}"
                        }
                    ]
                },
                {
                    "type": "rich_text_quote",
                    "elements": [
                        {
                            "type": "text",
                            "text": "This is a blockquoted user message"
                        }
                    ]
                },
                {
                    "type": "rich_text_list",
                    "style": "bullet",
                    "elements": [
                        {
                            "type": "rich_text_section",
                            "elements": [
                                { "type": "text", "text": "Bullet item one" }
                            ]
                        },
                        {
                            "type": "rich_text_section",
                            "elements": [
                                { "type": "text", "text": "Bullet item two (", "style": { "bold": true } },
                                { "type": "text", "text": "bold", "style": { "bold": true } },
                                { "type": "text", "text": ")" }
                            ]
                        }
                    ]
                }
            ]
        });

        match post_message_with_blocks(
            &client,
            bot_token,
            channel,
            "Block Kit test E fallback",
            &[rich_text_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test E posted (check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test E failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test F: chat.postMessage with `table` block (structured JSON) ───
    {
        let table_block = serde_json::json!({
            "type": "table",
            "columns": [
                { "id": "col_feature", "header": "Feature",  "width": 3 },
                { "id": "col_status",  "header": "Status",   "width": 2 },
                { "id": "col_notes",   "header": "Notes",    "width": 5 },
            ],
            "rows": [
                {
                    "id": "row_1",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Tables" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "plain_text", "text": "Native rendering via table block" },
                    }
                },
                {
                    "id": "row_2",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Code" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "plain_text", "text": "Fenced code blocks" },
                    }
                },
                {
                    "id": "row_3",
                    "cells": {
                        "col_feature": { "type": "plain_text", "text": "Links" },
                        "col_status":  { "type": "plain_text", "text": ":white_check_mark:" },
                        "col_notes":   { "type": "mrkdwn", "text": "<https://slack.com|Slack>" },
                    }
                },
            ]
        });

        match post_message_with_blocks(
            &client,
            bot_token,
            channel,
            "Block Kit test F fallback — table block",
            &[table_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test F posted (table block — check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test F failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // ── Test G: table block with alternate schema (rows as arrays) ──────
    // The docs showed two possible formats — try the array-of-arrays style
    {
        let table_block = serde_json::json!({
            "type": "table",
            "column_settings": [
                { "is_wrapped": true },
                { "align": "center" },
                { "align": "left", "is_wrapped": true },
            ],
            "rows": [
                [
                    { "type": "raw_text", "text": "Feature" },
                    { "type": "raw_text", "text": "Status" },
                    { "type": "raw_text", "text": "Notes" },
                ],
                [
                    { "type": "raw_text", "text": "Tables" },
                    { "type": "raw_text", "text": "✅" },
                    { "type": "raw_text", "text": "Native table block" },
                ],
                [
                    { "type": "raw_text", "text": "Code" },
                    { "type": "raw_text", "text": "✅" },
                    { "type": "raw_text", "text": "Fenced code blocks" },
                ],
                [
                    { "type": "raw_text", "text": "Links" },
                    { "type": "raw_text", "text": "✅" },
                    {
                        "type": "rich_text",
                        "elements": [{
                            "type": "rich_text_section",
                            "elements": [{
                                "type": "link",
                                "url": "https://slack.com",
                                "text": "Slack"
                            }]
                        }]
                    },
                ],
            ]
        });

        match post_message_with_blocks(
            &client,
            bot_token,
            channel,
            "Block Kit test G fallback — table block (array rows)",
            &[table_block],
            None,
            Some(thread_ts),
        )
        .await
        {
            Ok(_ts) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":white_check_mark: Test G posted (table block array format — check rendering above)",
                    Some(thread_ts),
                )
                .await;
            }
            Err(e) => {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    &format!(":x: Test G failed: {}", e),
                    Some(thread_ts),
                )
                .await;
            }
        }
    }

    let _ = post_message(
        &client,
        bot_token,
        channel,
        ":checkered_flag: Block Kit test battery complete. Check each test message above for rendering quality.",
        Some(thread_ts),
    )
    .await;
}

// ── Generic Command API Helper ─────────────────────────────────────────

/// Execute an OpenCode slash command via `POST /session/:id/command` and post
/// a success or failure message to the Slack thread.
async fn do_command_api(
    command: &str,
    arguments: &str,
    model: Option<&str>,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    success_msg: &str,
    error_prefix: &str,
) {
    let client = reqwest::Client::new();
    let api = crate::api::ApiClient::new();

    match api
        .execute_session_command(base_url, project_dir, session_id, command, arguments, model)
        .await
    {
        Ok(_resp) => {
            let _ = post_message(&client, bot_token, channel, success_msg, Some(thread_ts)).await;
            tracing::info!(
                "Slack @{}: executed for session {}",
                command,
                &session_id[..8.min(session_id.len())]
            );
        }
        Err(e) => {
            let msg = format!("{}: {}", error_prefix, e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            tracing::warn!(
                "Slack @{}: failed for session {}: {}",
                command,
                &session_id[..8.min(session_id.len())],
                e
            );
        }
    }
}

// ── New @ Command Handlers ─────────────────────────────────────────────

/// `@help` — List all available @ commands with descriptions.
async fn do_help_command(channel: &str, thread_ts: &str, bot_token: &str) {
    let client = reqwest::Client::new();

    let help_text = concat!(
        "*Available @ commands in this thread:*\n\n",
        "`@help` — Show this help message\n",
        "`@stop` — Cancel/abort the running session\n",
        "`@compact` — Summarize and compact the session\n",
        "`@undo` — Undo the last message and revert file changes\n",
        "`@redo` — Redo a previously undone message\n",
        "`@model` — List available models\n",
        "`@model <name>` — Switch to a different model\n",
        "`@status` — Show session status and info\n",
        "`@todos` — Show the session's todo list\n",
        "`@messages [N]` — Show the last N messages (default 5)\n",
        "`@export` — Export conversation to markdown\n",
        "`@watcher` — Enable continuation watcher (15s idle, 180s hang)\n",
        "`@watcher stop` — Disable the watcher\n",
        "`@detach` — Disconnect the relay from this thread\n",
        "\n_Any other `@<command>` is forwarded to OpenCode as a custom command._",
    );

    let _ = post_message(&client, bot_token, channel, help_text, Some(thread_ts)).await;
}

/// `@status` — Show session status (busy/idle), title, project, and relay info.
async fn do_status_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_idx: usize,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();
    let api = crate::api::ApiClient::new();

    // Fetch session status (busy/idle).
    let status_label = match api.fetch_session_status(base_url, project_dir).await {
        Ok(statuses) => {
            if let Some(st) = statuses.get(session_id) {
                match st.as_str() {
                    "busy" => ":large_green_circle: *busy*",
                    "retry" => ":yellow_circle: *retrying*",
                    _ => ":white_circle: *idle*",
                }
            } else {
                ":white_circle: *idle*"
            }
        }
        Err(_) => ":question: *unknown*",
    };

    // Fetch session info for title.
    let (title, created, updated) = match api.fetch_sessions(base_url, project_dir).await {
        Ok(sessions) => {
            if let Some(s) = sessions.iter().find(|s| s.id == session_id) {
                let t = if s.title.is_empty() {
                    "(untitled)".to_string()
                } else {
                    s.title.clone()
                };
                let created_str = if s.time.created > 0 {
                    chrono::DateTime::from_timestamp(s.time.created as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    "unknown".to_string()
                };
                let updated_str = if s.time.updated > 0 {
                    chrono::DateTime::from_timestamp(s.time.updated as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    "unknown".to_string()
                };
                (t, created_str, updated_str)
            } else {
                (
                    "(not found)".to_string(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                )
            }
        }
        Err(_) => (
            "(fetch error)".to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
        ),
    };

    // Check relay/watcher state.
    let s = slack_state.lock().await;
    let has_relay = s.relay_abort_handles.contains_key(session_id);
    let has_stream = s.streaming_messages.contains_key(session_id);
    let msg_offset = s.session_msg_offset.get(session_id).copied().unwrap_or(0);
    drop(s);

    let short_id = &session_id[..12.min(session_id.len())];
    let relay_icon = if has_relay {
        ":satellite:"
    } else {
        ":no_entry_sign:"
    };
    let stream_icon = if has_stream { ":zap:" } else { ":zzz:" };

    let msg = format!(
        concat!(
            "*Session Status*\n\n",
            ":label: *Title:* {title}\n",
            ":id: *ID:* `{short_id}`\n",
            ":signal_strength: *Status:* {status}\n",
            ":file_folder: *Project:* index {pidx} — `{pdir}`\n",
            ":calendar: *Created:* {created}\n",
            ":clock1: *Updated:* {updated}\n",
            "{relay_icon} *Relay:* {relay}\n",
            "{stream_icon} *Stream:* {stream}\n",
            ":1234: *Message offset:* {offset}",
        ),
        title = title,
        short_id = short_id,
        status = status_label,
        pidx = project_idx,
        pdir = project_dir,
        created = created,
        updated = updated,
        relay_icon = relay_icon,
        relay = if has_relay { "active" } else { "not attached" },
        stream_icon = stream_icon,
        stream = if has_stream { "active" } else { "inactive" },
        offset = msg_offset,
    );

    let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
}

/// `@todos` — Fetch and display the session's todo list.
async fn do_todos_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    let client = reqwest::Client::new();
    let api = crate::api::ApiClient::new();

    match api.fetch_todos(base_url, project_dir, session_id).await {
        Ok(todos) => {
            if todos.is_empty() {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    "*Session Todos*\n_No items._",
                    Some(thread_ts),
                )
                .await;
                return;
            }

            let mut lines = vec![format!("*Session Todos* ({} items)\n", todos.len())];
            for todo in &todos {
                let checkbox = match todo.status.as_str() {
                    "completed" => "- [x]",
                    "in_progress" => "- [-]",
                    "cancelled" => "- [~]",
                    _ => "- [ ]", // pending
                };
                let priority_tag = match todo.priority.as_str() {
                    "high" => "  `[HIGH]`",
                    "medium" => "  `[MED]`",
                    _ => "",
                };
                lines.push(format!("{} {}{}", checkbox, todo.content, priority_tag));
            }

            let done = todos.iter().filter(|t| t.status == "completed").count();
            lines.push(format!("\n_{}/{} completed_", done, todos.len()));

            let msg = lines.join("\n");
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
        }
        Err(e) => {
            let msg = format!("Failed to fetch todos: {}", e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
        }
    }
}

/// `@detach` — Disconnect the relay watcher from this thread.
async fn do_detach_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    bot_token: &str,
    slack_state: &Arc<Mutex<SlackState>>,
) {
    let client = reqwest::Client::new();

    let mut s = slack_state.lock().await;
    if let Some(_old_sid) = s.detach_relay(thread_ts) {
        drop(s);
        let msg = format!(
            ":wave: Detached relay for session `{}` from this thread. Messages will no longer be relayed here.",
            &session_id[..8.min(session_id.len())]
        );
        let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
        tracing::info!(
            "Slack @detach: detached session {} from thread",
            &session_id[..8.min(session_id.len())]
        );
    } else {
        drop(s);
        let _ = post_message(
            &client,
            bot_token,
            channel,
            ":information_source: No relay is attached to this thread.",
            Some(thread_ts),
        )
        .await;
    }
}

/// `@messages [N]` — Show the last N messages from the session (default 5).
async fn do_messages_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    args: &str,
) {
    let client = reqwest::Client::new();

    let count: usize = args.trim().parse().unwrap_or(5);
    let count = count.min(20); // cap at 20

    match super::tools::fetch_session_messages_with_tools(
        &client,
        base_url,
        project_dir,
        session_id,
    )
    .await
    {
        Ok(msgs) => {
            if msgs.is_empty() {
                let _ = post_message(
                    &client,
                    bot_token,
                    channel,
                    ":speech_balloon: No messages in this session yet.",
                    Some(thread_ts),
                )
                .await;
                return;
            }

            let total = msgs.len();
            let start = if total > count { total - count } else { 0 };
            let slice = &msgs[start..];

            let mut lines = vec![format!("*Last {} of {} messages:*\n", slice.len(), total)];

            for (i, msg) in slice.iter().enumerate() {
                let role_icon = match msg.role.as_str() {
                    "user" => ":bust_in_silhouette:",
                    "assistant" => ":robot_face:",
                    "system" => ":gear:",
                    _ => ":grey_question:",
                };

                // Truncate long messages for readability.
                let text = if msg.text.len() > 300 {
                    format!("{}…", &msg.text[..300])
                } else {
                    msg.text.clone()
                };

                let tool_count = msg.tools.len();
                let tool_suffix = if tool_count > 0 {
                    format!(
                        " _({} tool call{})_",
                        tool_count,
                        if tool_count == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                };

                lines.push(format!(
                    "{}. {} {}{}\n",
                    start + i + 1,
                    role_icon,
                    text.replace('\n', " "),
                    tool_suffix,
                ));
            }

            let msg = lines.join("\n");
            // Slack has a 3000-char limit per message; split if needed.
            if msg.len() > 3000 {
                let _ =
                    post_message(&client, bot_token, channel, &msg[..3000], Some(thread_ts)).await;
            } else {
                let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            }
        }
        Err(e) => {
            let msg = format!(":x: Failed to fetch messages: {}", e);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
        }
    }
}

/// `@model [name]` — Without arguments, list available models. With an argument,
/// switch the session's model via the command API.
async fn do_model_command(
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
    args: &str,
) {
    let client = reqwest::Client::new();
    let api = crate::api::ApiClient::new();
    let model_name = args.trim();

    if model_name.is_empty() {
        // List available models.
        match api.fetch_providers(base_url, project_dir).await {
            Ok(providers) => {
                let mut lines = vec!["*Available Models:*\n".to_string()];
                if let Some(arr) = providers.as_array() {
                    for provider in arr {
                        let pname = provider
                            .get("id")
                            .or_else(|| provider.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                            if models.is_empty() {
                                continue;
                            }
                            lines.push(format!("*{}*", pname));
                            for (model_id, model_info) in models {
                                let ctx = model_info
                                    .pointer("/limit/context")
                                    .and_then(|c| c.as_u64())
                                    .map(|c| format!(" ({}k ctx)", c / 1000))
                                    .unwrap_or_default();
                                lines.push(format!("  `{}`{}", model_id, ctx));
                            }
                            lines.push(String::new());
                        }
                    }
                }

                if lines.len() <= 1 {
                    lines.push("No providers/models found.".to_string());
                }

                let msg = lines.join("\n");
                if msg.len() > 3000 {
                    let _ =
                        post_message(&client, bot_token, channel, &msg[..3000], Some(thread_ts))
                            .await;
                } else {
                    let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
                }
            }
            Err(e) => {
                let msg = format!(":x: Failed to fetch models: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            }
        }
    } else {
        // Switch model via the command API.
        match api
            .execute_session_command(
                base_url,
                project_dir,
                session_id,
                "models",
                model_name,
                None,
            )
            .await
        {
            Ok(_) => {
                let msg = format!(
                    ":arrows_counterclockwise: Model switched to `{}`.",
                    model_name
                );
                let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
                tracing::info!(
                    "Slack @model: switched session {} to model {}",
                    &session_id[..8.min(session_id.len())],
                    model_name
                );
            }
            Err(e) => {
                let msg = format!(":x: Failed to switch model: {}", e);
                let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
                tracing::warn!(
                    "Slack @model: failed to switch session {} to {}: {}",
                    &session_id[..8.min(session_id.len())],
                    model_name,
                    e
                );
            }
        }
    }
}

/// Passthrough: attempt to execute an unrecognized `@<command>` as a custom
/// OpenCode command via the command API. If the server rejects it (404 or error),
/// show the `@help` output instead.
async fn do_passthrough_command(
    command: &str,
    arguments: &str,
    channel: &str,
    thread_ts: &str,
    session_id: &str,
    project_dir: &str,
    bot_token: &str,
    base_url: &str,
) {
    let api = crate::api::ApiClient::new();

    match api
        .execute_session_command(base_url, project_dir, session_id, command, arguments, None)
        .await
    {
        Ok(_) => {
            let client = reqwest::Client::new();
            let msg = format!(":white_check_mark: `@{}` executed.", command);
            let _ = post_message(&client, bot_token, channel, &msg, Some(thread_ts)).await;
            tracing::info!(
                "Slack passthrough @{}: executed for session {}",
                command,
                &session_id[..8.min(session_id.len())]
            );
        }
        Err(e) => {
            tracing::debug!(
                "Slack passthrough @{}: rejected for session {}: {}",
                command,
                &session_id[..8.min(session_id.len())],
                e
            );
            // Unknown command — show help.
            do_help_command(channel, thread_ts, bot_token).await;
        }
    }
}

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
                    let _ = super::api::post_to_response_url(
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
                    let _ = super::api::post_to_response_url(
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
                let _ = super::api::post_to_response_url(
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
                let _ = super::api::post_to_response_url(
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
                let _ = super::api::post_to_response_url(
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
            let _ = super::api::post_to_response_url(
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
        let _ = super::api::post_to_response_url(&client, response_url, &msg, true).await;
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

// ── Modal Builders ─────────────────────────────────────────────────────

/// Open a modal for `/opman-sessions` when invoked with no arguments.
///
/// Presents a dropdown of all project names. On submission, the selected
/// project is used to filter the sessions list.
async fn open_sessions_modal(
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

    super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
    Ok(())
}

/// Open step-1 modal for `/opman-connect`: project picker.
///
/// On submission (`callback_id: "opman_connect_project_modal"`), the selected
/// project is used to filter sessions for step 2.
async fn open_connect_project_modal(
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

    super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
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

    super::api::open_modal(&client, bot_token, trigger_id, &view).await?;
    Ok(())
}
