//! Session todos and context window usage handlers.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::api::ApiClient;
use crate::app::base_url;

/// GET /api/session/:id/todos — fetch todos for a session.
pub async fn get_session_todos(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let todos = client
        .fetch_todos(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(todos))
}

/// PUT /api/session/:id/todos — replace all todos for a session (full-replace semantics).
///
/// Accepts a JSON array of `TodoItem` objects. Writes them to the opencode
/// SQLite database using the same logic as the TUI's todo panel.
pub async fn update_session_todos(
    State(_state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(todos): Json<Vec<crate::app::TodoItem>>,
) -> WebResult<impl IntoResponse> {
    let sid = session_id.clone();
    let todo_list = todos.clone();
    tokio::task::spawn_blocking(move || {
        crate::todo_db::save_todos_to_db(&sid, &todo_list)
    })
    .await
    .map_err(|e| WebError::Internal(format!("join error: {e}")))?
    .map_err(|e| WebError::Internal(format!("Failed to save todos: {e}")))?;
    Ok(Json(todos))
}

/// Query params for `GET /api/context-window?session_id=...`.
#[derive(serde::Deserialize)]
pub struct ContextWindowQuery {
    /// Session ID to get context usage for.
    pub session_id: Option<String>,
}

/// GET /api/context-window — get context window usage breakdown.
///
/// Returns the context limit for the active model and a breakdown of
/// token usage by category (input, output, reasoning, cache).
pub async fn get_context_window(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<ContextWindowQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // 1. Determine which session to inspect
    let session_id = match query.session_id {
        Some(sid) => sid,
        None => state
            .web_state
            .active_session_id()
            .await
            .ok_or(WebError::BadRequest("No active session".into()))?,
    };

    // 2. Get session stats (already tracked via SSE)
    let stats = state
        .web_state
        .get_session_stats(&session_id)
        .await
        .unwrap_or_default();

    let total_used = stats.input_tokens
        + stats.output_tokens
        + stats.reasoning_tokens
        + stats.cache_read
        + stats.cache_write;

    // 3. Get context limit from providers
    let context_limit = {
        let client = ApiClient::with_client(state.http_client.clone());
        // Fetch providers to find the max context window
        let providers_result = client.fetch_providers(&base, &dir).await;
        match providers_result {
            Ok(providers_val) => {
                // providers_val is a serde_json::Value
                // Extract the default model's context limit, or find the max
                let mut max_context: u64 = 0;
                if let Some(all) = providers_val.get("all").and_then(|v| v.as_array()) {
                    for provider in all {
                        if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                            for (_model_id, model_info) in models {
                                if let Some(ctx) = model_info
                                    .pointer("/limit/context")
                                    .and_then(|c| c.as_u64())
                                {
                                    if ctx > max_context {
                                        max_context = ctx;
                                    }
                                }
                            }
                        }
                    }
                }
                // Also try the flat array format
                if max_context == 0 {
                    if let Some(arr) = providers_val.as_array() {
                        for provider in arr {
                            if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                                for (_model_id, model_info) in models {
                                    if let Some(ctx) = model_info
                                        .pointer("/limit/context")
                                        .and_then(|c| c.as_u64())
                                    {
                                        if ctx > max_context {
                                            max_context = ctx;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if max_context > 0 { max_context } else { 200_000 }
            }
            Err(_) => 200_000, // Fallback
        }
    };

    let usage_pct = if context_limit > 0 {
        (total_used as f64 / context_limit as f64) * 100.0
    } else {
        0.0
    };

    // 4. Build category breakdown from stats
    let mut categories = Vec::new();

    if stats.input_tokens > 0 {
        categories.push(ContextCategory {
            name: "input".to_string(),
            label: "Input Tokens".to_string(),
            tokens: stats.input_tokens,
            pct: if context_limit > 0 { (stats.input_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "blue".to_string(),
            items: vec![],
        });
    }

    if stats.output_tokens > 0 {
        categories.push(ContextCategory {
            name: "output".to_string(),
            label: "Output Tokens".to_string(),
            tokens: stats.output_tokens,
            pct: if context_limit > 0 { (stats.output_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "green".to_string(),
            items: vec![],
        });
    }

    if stats.reasoning_tokens > 0 {
        categories.push(ContextCategory {
            name: "reasoning".to_string(),
            label: "Reasoning Tokens".to_string(),
            tokens: stats.reasoning_tokens,
            pct: if context_limit > 0 { (stats.reasoning_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "purple".to_string(),
            items: vec![],
        });
    }

    if stats.cache_read > 0 || stats.cache_write > 0 {
        let cache_total = stats.cache_read + stats.cache_write;
        let mut items = Vec::new();
        if stats.cache_read > 0 {
            items.push(ContextItem {
                label: "Cache Read".to_string(),
                tokens: stats.cache_read,
            });
        }
        if stats.cache_write > 0 {
            items.push(ContextItem {
                label: "Cache Write".to_string(),
                tokens: stats.cache_write,
            });
        }
        categories.push(ContextCategory {
            name: "cache".to_string(),
            label: "Cache".to_string(),
            tokens: cache_total,
            pct: if context_limit > 0 { (cache_total as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "gray".to_string(),
            items,
        });
    }

    // 5. Estimate remaining messages
    // Use average tokens per message pair to estimate remaining capacity
    let estimated_messages_remaining = if total_used > 0 && context_limit > total_used {
        // Fetch message count to calculate average
        let remaining = context_limit - total_used;
        // Rough heuristic: count messages via the stats
        // Average input per exchange ~ input_tokens / max(1, number_of_exchanges)
        // Since we don't have message count here, use a simple heuristic
        let avg_per_exchange = if stats.input_tokens > 0 {
            // Assume input_tokens is split across ~N exchanges, each response
            // generates roughly equal output. Simple estimate: total / 2
            total_used / 2 // very rough: each exchange = total_so_far / messages
        } else {
            10_000 // default assumption: 10K tokens per exchange
        };
        if avg_per_exchange > 0 {
            Some(remaining / avg_per_exchange)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(ContextWindowResponse {
        context_limit,
        total_used,
        usage_pct,
        categories,
        estimated_messages_remaining,
    }))
}
