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
    /// Subagent ids (`agentId` values) referenced by `task` tool parts in this
    /// transcript — the live tailer streams each one's transcript as a child session.
    pub subagent_ids: Vec<String>,
}

/// Whether a `user` line is actually a harness/system injection (task-notification,
/// system-reminder, local-command echo) rather than a genuine human prompt.
/// Conservative: defaults to "human prompt" unless clearly a system injection.
fn is_system_injection(line: &Value, content: &str) -> bool {
    let ps = line.get("promptSource").and_then(|x| x.as_str());
    let kind = line
        .get("origin")
        .and_then(|o| o.get("kind"))
        .and_then(|k| k.as_str());
    if ps == Some("typed") || kind == Some("human") {
        return false;
    }
    if ps == Some("system") || matches!(kind, Some(k) if k != "human") {
        return true;
    }
    let t = content.trim_start();
    t.starts_with("<task-notification>")
        || t.starts_with("<system-reminder")
        || t.starts_with("<local-command")
        || t.starts_with("<command-name>")
}

/// Produce a clean, compact label for a system-injection bubble.
fn notification_text(content: &str) -> String {
    // Prefer a task-notification's <summary>…</summary>.
    if let (Some(a), Some(b)) = (content.find("<summary>"), content.find("</summary>")) {
        if b > a {
            let s = content[a + "<summary>".len()..b].trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    let t = content.trim();
    if t.chars().count() > 280 {
        let truncated: String = t.chars().take(280).collect();
        format!("{truncated}…")
    } else {
        t.to_string()
    }
}

fn iso_to_ms(s: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp_millis().max(0) as u64)
        .unwrap_or(0)
}

/// Stamp an assistant message's `time.completed` (once) so the UI stops treating it as
/// in-flight. No-op if the index is stale or it was already completed.
fn mark_completed(messages: &mut [MsgOut], idx: Option<usize>, ts: u64) {
    let Some(idx) = idx else { return };
    if let Some(time) = messages
        .get_mut(idx)
        .and_then(|m| m.info.get_mut("time"))
        .and_then(|t| t.as_object_mut())
    {
        time.entry("completed").or_insert(json!(ts));
    }
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

/// Cheaply read the latest `ai-title` from a transcript without a full parse.
pub fn read_ai_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut title = None;
    for line in content.lines() {
        if !line.contains("\"ai-title\"") {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if v.get("type").and_then(|t| t.as_str()) == Some("ai-title") {
                if let Some(t) = v.get("aiTitle").and_then(|t| t.as_str()) {
                    if !t.trim().is_empty() {
                        title = Some(t.trim().to_string());
                    }
                }
            }
        }
    }
    title
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
    let mut sys_turn: usize = 0;
    // Index of the most recent assistant message; marked `time.completed` once its turn
    // ends (a `turn_duration` system line, a following genuine user prompt, or a new
    // assistant message supersedes it). The trailing assistant of an in-flight turn
    // stays uncompleted, which is exactly what the web UI's "Queued" badge keys off.
    let mut last_assistant_idx: Option<usize> = None;

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
                // A genuine prompt or a system injection marks the end of the prior
                // assistant turn. A tool_result (array content) is mid-turn — skip it.
                if !matches!(content_v, Some(Value::Array(_))) {
                    mark_completed(&mut out.messages, last_assistant_idx.take(), ts);
                }
                match content_v {
                    Some(Value::String(s)) if is_system_injection(&v, s) => {
                        // A harness/system injection (task-notification, system-reminder,
                        // local-command echo) — render as a distinct system bubble, not a
                        // user message.
                        sys_turn += 1;
                        let mid = format!("msg_sys_{session_id}_{sys_turn}");
                        out.messages.push(MsgOut {
                            info: json!({
                                "role": "system",
                                "variant": "notification",
                                "id": mid,
                                "sessionID": session_id,
                                "time": { "created": ts },
                            }),
                            parts: vec![json!({
                                "type": "text",
                                "id": format!("{mid}:0"),
                                "messageID": mid,
                                "sessionID": session_id,
                                "text": notification_text(s),
                            })],
                        });
                    }
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
                                            let is_task = part
                                                .get("tool")
                                                .and_then(|t| t.as_str())
                                                == Some("task");
                                            if let Some(state) =
                                                part.get_mut("state").and_then(|s| s.as_object_mut())
                                            {
                                                if is_task {
                                                    // Async launch ack carries the child
                                                    // `agentId`; record it so the UI can
                                                    // stream the subagent. Leave status as
                                                    // "running" — `enrich_subagents` decides
                                                    // completion from the child transcript.
                                                    if let Some(aid) = parse_agent_id(&output) {
                                                        let meta = state
                                                            .entry("metadata")
                                                            .or_insert_with(|| json!({}));
                                                        if let Some(m) = meta.as_object_mut() {
                                                            m.insert("sessionId".into(), json!(aid));
                                                        }
                                                    }
                                                } else {
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
                        // A new assistant message supersedes the previous one (e.g. the
                        // tool_use message before this text message) — complete it.
                        mark_completed(&mut out.messages, last_assistant_idx.take(), ts);
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
                last_assistant_idx = Some(idx);
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
                                let raw_name = b
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("tool");
                                // claude's subagent launchers (`Agent`/`Task`) map to
                                // opencode's `task` tool so the web UI renders them as
                                // an inline collapsible subagent session.
                                let is_task = raw_name == "Agent" || raw_name == "Task";
                                let name = if is_task { "task" } else { raw_name };
                                let input = b.get("input").cloned().unwrap_or(json!({}));
                                let title = if is_task {
                                    input
                                        .get("description")
                                        .and_then(|d| d.as_str())
                                        .or_else(|| input.get("subagent_type").and_then(|s| s.as_str()))
                                        .unwrap_or("Task")
                                        .to_string()
                                } else {
                                    name.to_string()
                                };
                                let mut state = json!({
                                    "input": input,
                                    "status": "running",
                                    "title": title,
                                    "time": { "start": ts },
                                });
                                // A `task` part carries the child session id in
                                // `state.metadata.sessionId`; the launch-ack tool_result
                                // fills it in (see the tool_result branch below).
                                if is_task {
                                    state["metadata"] = json!({});
                                }
                                out.messages[idx].parts.push(json!({
                                    "type": "tool",
                                    "id": tid,
                                    "callID": tid,
                                    "tool": name,
                                    "messageID": mid,
                                    "sessionID": session_id,
                                    "state": state,
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
            "system" => {
                // `turn_duration` is written when a turn finishes — complete its assistant.
                if v.get("subtype").and_then(|s| s.as_str()) == Some("turn_duration") {
                    let ts = v
                        .get("timestamp")
                        .and_then(|t| t.as_str())
                        .map(iso_to_ms)
                        .unwrap_or(0);
                    mark_completed(&mut out.messages, last_assistant_idx.take(), ts);
                }
            }
            _ => {}
        }
    }

    // Collect child agent ids referenced by `task` parts (for live subagent tailing).
    for msg in &out.messages {
        for part in &msg.parts {
            if part.get("tool").and_then(|t| t.as_str()) != Some("task") {
                continue;
            }
            if let Some(aid) = part
                .get("state")
                .and_then(|s| s.get("metadata"))
                .and_then(|m| m.get("sessionId"))
                .and_then(|v| v.as_str())
            {
                let aid = aid.to_string();
                if !out.subagent_ids.contains(&aid) {
                    out.subagent_ids.push(aid);
                }
            }
        }
    }

    out
}

/// Parse the `agentId: <id>` token out of an async-subagent launch ack.
fn parse_agent_id(s: &str) -> Option<String> {
    let i = s.find("agentId:")?;
    let rest = s[i + "agentId:".len()..].trim_start();
    let id: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    (!id.is_empty()).then_some(id)
}

/// Whether a subagent transcript has finished (ends with a final assistant text answer,
/// i.e. no tool call awaiting a result). Returns the final text when complete.
pub fn subagent_completed(sub: &ParsedSession) -> (bool, Option<String>) {
    let Some(last) = sub.messages.last() else {
        return (false, None);
    };
    if last.info.get("role").and_then(|r| r.as_str()) != Some("assistant") {
        return (false, None);
    }
    match last.parts.last() {
        Some(p) if p.get("type").and_then(|t| t.as_str()) == Some("text") => {
            let txt = p.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            (true, Some(txt))
        }
        _ => (false, None),
    }
}

/// Fill in each `task` part's running/completed status (and final output) from the
/// corresponding subagent transcript on disk. Parsing is pure; this step does fs lookups
/// and is called by the live tailer and the REST message endpoints.
pub fn enrich_subagents(out: &mut ParsedSession) {
    for msg in &mut out.messages {
        for part in &mut msg.parts {
            if part.get("tool").and_then(|t| t.as_str()) != Some("task") {
                continue;
            }
            let aid = part
                .get("state")
                .and_then(|s| s.get("metadata"))
                .and_then(|m| m.get("sessionId"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let Some(aid) = aid else {
                continue;
            };
            let (status, output) = match super::claude_cli::locate_subagent_jsonl(&aid) {
                Some(path) => {
                    let sub = parse_file(&path, &aid);
                    let (done, text) = subagent_completed(&sub);
                    if done {
                        ("completed", text)
                    } else {
                        ("running", None)
                    }
                }
                // Launched but the child transcript hasn't appeared yet — still running.
                None => ("running", None),
            };
            if let Some(state) = part.get_mut("state").and_then(|s| s.as_object_mut()) {
                state.insert("status".into(), json!(status));
                if let Some(text) = output {
                    state.insert("output".into(), json!(text));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors claude v2.1.195: an `Agent` tool_use whose tool_result is the async
    // launch ack carrying `agentId`. We map it to opencode's `task` tool and lift the
    // agentId into state.metadata.sessionId so the web UI streams the subagent.
    #[test]
    fn agent_tool_maps_to_task_with_child_id() {
        let transcript = concat!(
            r#"{"type":"assistant","timestamp":"2026-06-28T08:22:00.000Z","message":{"id":"msg_1","model":"claude-haiku","content":[{"type":"tool_use","id":"toolu_1","name":"Agent","input":{"description":"Count files","prompt":"count"}}]}}"#, "\n",
            r#"{"type":"user","timestamp":"2026-06-28T08:22:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"Async agent launched successfully.\nagentId: a1834b2decb148144 (internal ID - do not mention)"}]}]}}"#, "\n",
        );
        let parsed = parse_str(transcript, "ses_test");
        let part = &parsed.messages[0].parts[0];
        assert_eq!(part["tool"], "task");
        assert_eq!(part["state"]["title"], "Count files");
        assert_eq!(part["state"]["metadata"]["sessionId"], "a1834b2decb148144");
        // Stays "running" until enrich_subagents consults the child transcript.
        assert_eq!(part["state"]["status"], "running");
        assert_eq!(parsed.subagent_ids, vec!["a1834b2decb148144".to_string()]);
    }

    #[test]
    fn ordinary_tool_result_still_completes() {
        let transcript = concat!(
            r#"{"type":"assistant","timestamp":"2026-06-28T08:22:00.000Z","message":{"id":"msg_1","content":[{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"ls"}}]}}"#, "\n",
            r#"{"type":"user","timestamp":"2026-06-28T08:22:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_2","content":[{"type":"text","text":"file.txt"}]}]}}"#, "\n",
        );
        let parsed = parse_str(transcript, "ses_test");
        let part = &parsed.messages[0].parts[0];
        assert_eq!(part["tool"], "Bash");
        assert_eq!(part["state"]["status"], "completed");
        assert_eq!(part["state"]["output"], "file.txt");
        assert!(parsed.subagent_ids.is_empty());
    }

    // A finished turn (ends with a turn_duration line) marks the assistant completed,
    // so the UI's "Queued" badge logic sees no pending assistant. An in-flight turn
    // (no turn_duration yet) leaves it uncompleted.
    #[test]
    fn assistant_completed_on_turn_end_but_not_while_streaming() {
        let finished = concat!(
            r#"{"type":"user","timestamp":"2026-06-28T08:00:00.000Z","promptSource":"typed","message":{"role":"user","content":"hi"}}"#, "\n",
            r#"{"type":"assistant","timestamp":"2026-06-28T08:00:01.000Z","message":{"id":"msg_01","content":[{"type":"text","text":"hello"}]}}"#, "\n",
            r#"{"type":"system","subtype":"turn_duration","timestamp":"2026-06-28T08:00:02.000Z"}"#, "\n",
        );
        let p = parse_str(finished, "ses");
        let asst = p.messages.iter().find(|m| m.info["role"] == "assistant").unwrap();
        assert!(asst.info["time"].get("completed").is_some(), "finished turn should be completed");

        let streaming = concat!(
            r#"{"type":"user","timestamp":"2026-06-28T08:00:00.000Z","promptSource":"typed","message":{"role":"user","content":"hi"}}"#, "\n",
            r#"{"type":"assistant","timestamp":"2026-06-28T08:00:01.000Z","message":{"id":"msg_01","content":[{"type":"text","text":"thinking..."}]}}"#, "\n",
        );
        let p = parse_str(streaming, "ses");
        let asst = p.messages.iter().find(|m| m.info["role"] == "assistant").unwrap();
        assert!(asst.info["time"].get("completed").is_none(), "streaming turn stays in-flight");
    }

    #[test]
    fn parses_agent_id_token() {
        assert_eq!(
            parse_agent_id("Async agent launched successfully.\nagentId: a1834b2decb148144 (internal)"),
            Some("a1834b2decb148144".to_string())
        );
        assert_eq!(parse_agent_id("no id here"), None);
    }
}
