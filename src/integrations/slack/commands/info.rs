//! Info commands: help, status, todos, messages.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::super::api::post_message;
use super::super::state::SlackState;

/// `@help` — List all available @ commands with descriptions.
pub(super) async fn do_help_command(channel: &str, thread_ts: &str, bot_token: &str) {
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
pub(super) async fn do_status_command(
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
pub(super) async fn do_todos_command(
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

/// `@messages [N]` — Show the last N messages from the session (default 5).
pub(super) async fn do_messages_command(
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

    match super::super::tools::fetch_session_messages_with_tools(
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
