//! Tool implementations for the MCP kanban server. Each tool makes an
//! authenticated HTTP call to the web server's loopback `/internal/kanban` API.

use serde_json::{json, Value};

use super::Internal;

pub(super) async fn dispatch_tool(internal: Option<&Internal>, params: Option<Value>) -> String {
    let Some(internal) = internal else {
        return "Kanban API is unavailable (the opman web server is not running, or ~/.config/opman/internal.json is missing).".to_string();
    };
    let params = params.unwrap_or(json!({}));
    let tool = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    if task_id.is_empty() {
        return "Missing required argument: task_id".to_string();
    }

    match tool {
        "kanban_get_task" => get(internal, &format!("/internal/kanban/task/{task_id}")).await,
        "kanban_set_lane" => {
            let lane = args.get("lane").and_then(|v| v.as_str()).unwrap_or("");
            if lane.is_empty() {
                return "Missing required argument: lane".to_string();
            }
            post(
                internal,
                &format!("/internal/kanban/task/{task_id}/status"),
                json!({ "lane": lane, "run_state": "running" }),
            )
            .await
        }
        "kanban_add_note" => {
            let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
            if body.is_empty() {
                return "Missing required argument: body".to_string();
            }
            post(
                internal,
                &format!("/internal/kanban/task/{task_id}/note"),
                json!({ "body": body }),
            )
            .await
        }
        "kanban_complete" => {
            let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            post(
                internal,
                &format!("/internal/kanban/task/{task_id}/complete"),
                json!({ "body": summary }),
            )
            .await
        }
        "kanban_list_tasks" => {
            let body = json!({
                "lane": args.get("lane").and_then(|v| v.as_str()),
                "tags": args.get("tags").cloned().unwrap_or(json!([])),
                "query": args.get("query").and_then(|v| v.as_str()),
                "include_archived": args
                    .get("include_archived")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
            post(internal, &format!("/internal/kanban/task/{task_id}/query"), body).await
        }
        "kanban_board_summary" => {
            get(internal, &format!("/internal/kanban/task/{task_id}/board")).await
        }
        "kanban_read_notes" => {
            let body = json!({ "task_ids": args.get("task_ids").cloned().unwrap_or(json!([])) });
            post(internal, &format!("/internal/kanban/task/{task_id}/notes"), body).await
        }
        other => format!("Unknown tool: {other}"),
    }
}

async fn get(internal: &Internal, path: &str) -> String {
    let res = internal
        .client
        .get(format!("{}{}", internal.url, path))
        .header("x-internal-token", &internal.token)
        .send()
        .await;
    handle(res).await
}

async fn post(internal: &Internal, path: &str, body: Value) -> String {
    let res = internal
        .client
        .post(format!("{}{}", internal.url, path))
        .header("x-internal-token", &internal.token)
        .json(&body)
        .send()
        .await;
    handle(res).await
}

async fn handle(res: Result<reqwest::Response, reqwest::Error>) -> String {
    match res {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                text
            } else {
                format!("Error {}: {}", status.as_u16(), text)
            }
        }
        Err(e) => format!("Request failed: {e}"),
    }
}
