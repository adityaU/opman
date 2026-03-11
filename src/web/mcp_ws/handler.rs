//! WebSocket upgrade handler, MCP session loop, and method dispatch.

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tracing::{debug, warn};

use crate::web::auth::check_auth_manual;
use crate::web::error::WebError;
use crate::web::types::{ServerState, SseTokenQuery, WebEvent};

use super::editor::{handle_editor_list, handle_editor_open, handle_editor_read};
use super::protocol::{
    JsonRpcRequest, JsonRpcResponse, INVALID_REQUEST, METHOD_NOT_FOUND, MCP_PROTOCOL_VERSION,
    PARSE_ERROR, SERVER_NAME, SERVER_VERSION,
};
use super::terminal::{
    handle_terminal_close, handle_terminal_list, handle_terminal_new, handle_terminal_read,
    handle_terminal_run,
};
use super::tools::web_mcp_tool_definitions;

// ── WebSocket handler ───────────────────────────────────────────────

/// WebSocket upgrade handler for MCP connections.
///
/// `GET /api/mcp/ws?token=<jwt>`
pub async fn websocket_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(params): Query<SseTokenQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WebError> {
    if !check_auth_manual(&state, &headers, &params.token) {
        return Err(WebError::Unauthorized);
    }

    Ok(ws.on_upgrade(move |socket| handle_mcp_session(socket, state)))
}

/// Main MCP session loop — reads JSON-RPC requests, dispatches, sends responses.
async fn handle_mcp_session(socket: WebSocket, state: ServerState) {
    let (mut sender, mut receiver) = socket.split();

    debug!("MCP WebSocket client connected");

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => {
                debug!("MCP WebSocket client disconnected (close frame)");
                break;
            }
            Ok(_) => continue, // Ignore binary, ping, pong
            Err(e) => {
                warn!("MCP WebSocket error: {}", e);
                break;
            }
        };

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    None,
                    PARSE_ERROR,
                    format!("JSON parse error: {}", e),
                );
                if let Ok(json) = serde_json::to_string(&resp) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse::error(
                request.id.clone(),
                INVALID_REQUEST,
                "Expected jsonrpc: \"2.0\"",
            );
            if let Ok(json) = serde_json::to_string(&resp) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            continue;
        }

        // Dispatch method
        let response = dispatch_method(&state, &request).await;

        // Notifications (no id) don't get responses per JSON-RPC spec
        if request.id.is_none() {
            continue;
        }

        if let Ok(json) = serde_json::to_string(&response) {
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    }

    debug!("MCP WebSocket session ended");
}

// ── Method dispatch ─────────────────────────────────────────────────

async fn dispatch_method(state: &ServerState, req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "initialized" => {
            // Notification — acknowledge silently
            JsonRpcResponse::success(req.id.clone(), serde_json::json!({}))
        }
        "tools/list" => handle_tools_list(req),
        "tools/call" => handle_tools_call(state, req).await,
        "ping" => JsonRpcResponse::success(req.id.clone(), serde_json::json!({})),
        _ => JsonRpcResponse::error(
            req.id.clone(),
            METHOD_NOT_FOUND,
            format!("Unknown method: {}", req.method),
        ),
    }
}

// ── MCP: initialize ─────────────────────────────────────────────────

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        }),
    )
}

// ── MCP: tools/list ─────────────────────────────────────────────────

fn handle_tools_list(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "tools": web_mcp_tool_definitions()
        }),
    )
}

// ── MCP: tools/call ─────────────────────────────────────────────────

async fn handle_tools_call(state: &ServerState, req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = req.params.as_ref().cloned().unwrap_or(serde_json::json!({}));
    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                INVALID_REQUEST,
                "Missing tool name in params.name",
            );
        }
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Emit agent activity event
    let _ = state.event_tx.send(WebEvent::McpAgentActivity {
        tool: tool_name.clone(),
        active: true,
    });

    let result = match tool_name.as_str() {
        "web_terminal_read" => handle_terminal_read(state, &arguments).await,
        "web_terminal_run" => handle_terminal_run(state, &arguments).await,
        "web_terminal_list" => handle_terminal_list(state).await,
        "web_terminal_new" => handle_terminal_new(state, &arguments).await,
        "web_terminal_close" => handle_terminal_close(state, &arguments).await,
        "web_editor_open" => handle_editor_open(state, &arguments).await,
        "web_editor_read" => handle_editor_read(state, &arguments).await,
        "web_editor_list" => handle_editor_list(state, &arguments).await,
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    // Emit agent activity off
    let _ = state.event_tx.send(WebEvent::McpAgentActivity {
        tool: tool_name,
        active: false,
    });

    match result {
        Ok(content) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": content
                }]
            }),
        ),
        Err(e) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": e
                }],
                "isError": true
            }),
        ),
    }
}
