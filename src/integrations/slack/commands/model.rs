//! Model command: list and switch models.

use super::super::api::post_message;

/// `@model [name]` — Without arguments, list available models. With an argument,
/// switch the session's model via the command API.
pub(super) async fn do_model_command(
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
