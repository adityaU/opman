//! Parse a claude session transcript (`<uuid>.jsonl`) into opencode-shaped messages.
//!
//! Persisted line shapes (claude v2.1.195):
//! - `{"type":"user","message":{"role":"user","content":"<str>"|[{type:"tool_result",…}]},…}`
//! - `{"type":"assistant","message":{"id":"msg_…","model":"…","content":[<ONE block>],"usage":{…}},
//!    "requestId":…,"timestamp":"<rfc3339>"}` — one block per line; lines sharing `message.id`
//!    form one logical assistant message.
//! - `{"type":"ai-title","aiTitle":"…"}` — session title.
//! - meta lines (`mode`, `permission-mode`, `file-history-snapshot`, `attachment`,
//!   `last-prompt`, `agent-name`, `system`) are ignored for message rendering.
//!
//! Output matches what opman + the web UI expect from `GET /session/{id}/message`:
//! an array of `{ "info": {...}, "parts": [...] }`, with opencode part `type`s
//! (`text`, `reasoning`, `tool`).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

/// One opencode-shaped message.
#[derive(Debug, Clone)]
pub struct MsgOut {
    pub info: Value,
    pub parts: Vec<Value>,
}

impl MsgOut {
    pub fn to_value(&self) -> Value {
        json!({ "info": self.info, "parts": self.parts })
    }
}

/// Result of parsing a transcript.
#[derive(Debug, Clone, Default)]
pub struct ParsedSession {
    pub messages: Vec<MsgOut>,
    pub title: Option<String>,
    pub model: Option<String>,
}

fn iso_to_ms(s: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp_millis().max(0) as u64)
        .unwrap_or(0)
}

fn stringify_content(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(|b| {
                if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                    t.to_string()
                } else if let Some(s) = b.as_str() {
                    s.to_string()
                } else {
                    b.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

/// Map claude usage → opencode `tokens` object.
fn tokens_from_usage(usage: &Value) -> Value {
    let g = |k: &str| usage.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    json!({
        "input": g("input_tokens"),
        "output": g("output_tokens"),
        "reasoning": usage
            .get("output_tokens_details")
            .and_then(|d| d.get("thinking_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        "cache": {
            "read": g("cache_read_input_tokens"),
            "write": g("cache_creation_input_tokens"),
        }
    })
}

/// Parse a transcript file into opencode-shaped messages.
pub fn parse_file(path: &Path, session_id: &str) -> ParsedSession {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ParsedSession::default(),
    };
    parse_str(&content, session_id)
}

/// Parse transcript text (split out for testing).
pub fn parse_str(content: &str, session_id: &str) -> ParsedSession {
    let mut out = ParsedSession::default();
    // assistant message.id → index into out.messages
    let mut assistant_idx: HashMap<String, usize> = HashMap::new();
    // tool_use id → (message index, part index) so tool_result lines can attach output
    let mut tool_loc: HashMap<String, (usize, usize)> = HashMap::new();
    let mut user_turn: usize = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ltype = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ltype {
            "ai-title" => {
                if let Some(t) = v.get("aiTitle").and_then(|t| t.as_str()) {
                    out.title = Some(t.to_string());
                }
            }
            "user" => {
                let msg = v.get("message");
                let content_v = msg.and_then(|m| m.get("content"));
                let ts = v
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .map(iso_to_ms)
                    .unwrap_or(0);
                match content_v {
                    Some(Value::String(s)) => {
                        // A genuine user prompt.
                        user_turn += 1;
                        let mid = format!("msg_user_{session_id}_{user_turn}");
                        out.messages.push(MsgOut {
                            info: json!({
                                "role": "user",
                                "id": mid,
                                "sessionID": session_id,
                                "time": { "created": ts },
                            }),
                            parts: vec![json!({
                                "type": "text",
                                "id": format!("{mid}:0"),
                                "messageID": mid,
                                "sessionID": session_id,
                                "text": s,
                            })],
                        });
                    }
                    Some(Value::Array(blocks)) => {
                        // Tool results: attach to the matching assistant tool part.
                        for b in blocks {
                            if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                if let Some(tid) =
                                    b.get("tool_use_id").and_then(|t| t.as_str())
                                {
                                    if let Some(&(mi, pi)) = tool_loc.get(tid) {
                                        let is_err = b
                                            .get("is_error")
                                            .and_then(|e| e.as_bool())
                                            .unwrap_or(false);
                                        let output = b
                                            .get("content")
                                            .map(stringify_content)
                                            .unwrap_or_default();
                                        if let Some(part) =
                                            out.messages.get_mut(mi).and_then(|m| m.parts.get_mut(pi))
                                        {
                                            if let Some(state) =
                                                part.get_mut("state").and_then(|s| s.as_object_mut())
                                            {
                                                state.insert("output".into(), json!(output));
                                                state.insert(
                                                    "status".into(),
                                                    json!(if is_err { "error" } else { "completed" }),
                                                );
                                                if is_err {
                                                    state.insert("error".into(), json!(output));
                                                }
                                                if let Some(time) = state
                                                    .get_mut("time")
                                                    .and_then(|t| t.as_object_mut())
                                                {
                                                    time.insert("end".into(), json!(ts));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                let msg = match v.get("message") {
                    Some(m) => m,
                    None => continue,
                };
                let mid = msg
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                if mid.is_empty() {
                    continue;
                }
                let ts = v
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .map(iso_to_ms)
                    .unwrap_or(0);
                let model = msg.get("model").and_then(|m| m.as_str()).map(String::from);
                if model.is_some() && out.model.is_none() {
                    out.model = model.clone();
                }

                // Get-or-create the assistant message.
                let idx = match assistant_idx.get(&mid) {
                    Some(&i) => i,
                    None => {
                        let info = json!({
                            "role": "assistant",
                            "id": mid,
                            "sessionID": session_id,
                            "model": model.clone().unwrap_or_default(),
                            "cost": 0.0,
                            "tokens": msg.get("usage").map(tokens_from_usage).unwrap_or(json!({})),
                            "time": { "created": ts },
                        });
                        out.messages.push(MsgOut { info, parts: vec![] });
                        let i = out.messages.len() - 1;
                        assistant_idx.insert(mid.clone(), i);
                        i
                    }
                };
                // Refresh tokens from the latest usage seen for this message.
                if let Some(usage) = msg.get("usage") {
                    if let Some(info) = out.messages.get_mut(idx).map(|m| &mut m.info) {
                        info["tokens"] = tokens_from_usage(usage);
                    }
                }

                let blocks = msg.get("content").and_then(|c| c.as_array());
                if let Some(blocks) = blocks {
                    for b in blocks {
                        let bt = b.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        let part_index = out.messages[idx].parts.len();
                        match bt {
                            "text" => {
                                if let Some(text) = b.get("text").and_then(|t| t.as_str()) {
                                    out.messages[idx].parts.push(json!({
                                        "type": "text",
                                        "id": format!("{mid}:{part_index}"),
                                        "messageID": mid,
                                        "sessionID": session_id,
                                        "text": text,
                                    }));
                                }
                            }
                            "thinking" => {
                                let text = b
                                    .get("thinking")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                out.messages[idx].parts.push(json!({
                                    "type": "reasoning",
                                    "id": format!("{mid}:{part_index}"),
                                    "messageID": mid,
                                    "sessionID": session_id,
                                    "text": text,
                                }));
                            }
                            "tool_use" => {
                                let tid = b
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = b
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("tool");
                                let input = b.get("input").cloned().unwrap_or(json!({}));
                                out.messages[idx].parts.push(json!({
                                    "type": "tool",
                                    "id": tid,
                                    "callID": tid,
                                    "tool": name,
                                    "messageID": mid,
                                    "sessionID": session_id,
                                    "state": {
                                        "input": input,
                                        "status": "running",
                                        "title": name,
                                        "time": { "start": ts },
                                    },
                                }));
                                if !tid.is_empty() {
                                    tool_loc.insert(tid, (idx, part_index));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    out
}
