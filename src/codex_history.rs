//! Read Codex rollout records that are not exposed by `thread/read`.
//!
//! The app-server exposes native edits and searches through `thread/read`, but
//! code-mode shell calls are persisted as custom `exec` records in the rollout
//! JSONL file. Keeping this adapter separate makes the runner protocol code
//! independent from the on-disk compatibility path.

use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde_json::{json, Value};

struct PendingCall {
    id: String,
    input: String,
    created: u64,
}

pub(crate) struct RolloutHistory {
    pub(crate) bash_messages: Vec<Value>,
    pub(crate) message_times: HashMap<String, VecDeque<u64>>,
}

/// Load the timestamped rollout events needed to complete native thread history.
pub(crate) fn load(session_id: &str) -> RolloutHistory {
    let Some(path) = find_rollout(session_id) else {
        return RolloutHistory::empty();
    };
    let Ok(file) = File::open(path) else {
        return RolloutHistory::empty();
    };
    parse_lines(BufReader::new(file).lines())
}

impl RolloutHistory {
    fn empty() -> Self {
        Self {
            bash_messages: Vec::new(),
            message_times: HashMap::new(),
        }
    }
}

fn find_rollout(session_id: &str) -> Option<PathBuf> {
    let root = dirs::home_dir()?.join(".codex").join("sessions");
    find_rollout_in(&root, session_id)
}

fn find_rollout_in(directory: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(directory).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_rollout_in(&path, session_id) {
                return Some(found);
            }
            continue;
        }
        let name = path.file_name().and_then(|name| name.to_str())?;
        if name.starts_with("rollout-") && name.ends_with(&format!("-{session_id}.jsonl")) {
            return Some(path);
        }
    }
    None
}

fn parse_lines<I>(lines: I) -> RolloutHistory
where
    I: IntoIterator<Item = io::Result<String>>,
{
    let mut calls = Vec::new();
    let mut outputs = HashMap::new();
    let mut message_times = HashMap::new();
    for line in lines.into_iter().filter_map(Result::ok) {
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(payload) = record.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("message") => {
                let role = payload.get("role").and_then(Value::as_str);
                if !matches!(role, Some("user") | Some("assistant")) {
                    continue;
                }
                let text = message_text(payload);
                let created = timestamp_ms(record.get("timestamp"));
                if !text.is_empty() && created > 0 {
                    message_times
                        .entry(text)
                        .or_insert_with(VecDeque::new)
                        .push_back(created);
                }
            }
            Some("custom_tool_call") => {
                let Some(input) = payload.get("input").and_then(Value::as_str) else {
                    continue;
                };
                if !input.contains("tools.exec_command") {
                    continue;
                }
                let Some(id) = payload.get("call_id").and_then(Value::as_str) else {
                    continue;
                };
                calls.push(PendingCall {
                    id: id.to_string(),
                    input: input.to_string(),
                    created: timestamp_ms(record.get("timestamp")),
                });
            }
            Some("custom_tool_call_output") => {
                let Some(id) = payload.get("call_id").and_then(Value::as_str) else {
                    continue;
                };
                outputs.insert(id.to_string(), output_text(payload));
            }
            _ => {}
        }
    }

    let bash_messages = calls
        .into_iter()
        .enumerate()
        .map(|(index, call)| {
            let output = outputs.remove(&call.id).unwrap_or_default();
            let status = if output.contains("Script failed") {
                "error"
            } else {
                "completed"
            };
            let created = if call.created == 0 {
                index as u64
            } else {
                call.created
            };
            json!({
                "info": {
                    "id": format!("codex_bash_{}", call.id),
                    "messageID": format!("codex_bash_{}", call.id),
                    "role": "assistant",
                    "time": { "created": created }
                },
                "parts": [{
                    "id": format!("{}_part", call.id),
                    "type": "tool",
                    "tool": "bash",
                    "callID": call.id,
                    "state": {
                        "status": status,
                        "input": { "command": "exec", "source": call.input },
                        "output": output
                    }
                }]
            })
        })
        .collect();
    RolloutHistory {
        bash_messages,
        message_times,
    }
}

pub(crate) fn annotate_native_messages(
    messages: &mut [Value],
    mut message_times: HashMap<String, VecDeque<u64>>,
) {
    for message in messages {
        let Some(role) = message.pointer("/info/role").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(role, "user" | "assistant") {
            continue;
        }
        let Some(text) = message
            .pointer("/parts/0/text")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let Some(created) = message_times.get_mut(text).and_then(VecDeque::pop_front) else {
            continue;
        };
        let Some(info) = message.get_mut("info").and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(time) = info.get_mut("time").and_then(Value::as_object_mut) else {
            continue;
        };
        time.insert("created".to_string(), Value::from(created));
    }
}

fn message_text(payload: &Value) -> String {
    payload
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| {
            let kind = part.get("type").and_then(Value::as_str)?;
            if !matches!(kind, "input_text" | "output_text") {
                return None;
            }
            part.get("text").and_then(Value::as_str)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn output_text(payload: &Value) -> String {
    payload
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn timestamp_ms(value: Option<&Value>) -> u64 {
    value
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .and_then(|value| u64::try_from(value.timestamp_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::parse_lines;

    #[test]
    fn maps_exec_rollout_records_to_bash_messages() {
        let lines = [
            r#"{"timestamp":"2026-08-03T12:00:00Z","payload":{"type":"custom_tool_call","call_id":"call-1","input":"const r = await tools.exec_command({cmd: \"pwd\"});"}}"#,
            r#"{"timestamp":"2026-08-03T12:00:01Z","payload":{"type":"custom_tool_call_output","call_id":"call-1","output":[{"type":"input_text","text":"/tmp"}]}}"#,
        ];
        let history = parse_lines(lines.into_iter().map(|line| Ok(line.to_string())));
        assert_eq!(history.bash_messages.len(), 1);
        assert_eq!(history.bash_messages[0]["parts"][0]["tool"], "bash");
        assert_eq!(
            history.bash_messages[0]["parts"][0]["state"]["output"],
            "/tmp"
        );
    }

    #[test]
    fn ignores_non_exec_custom_tools() {
        let line = r#"{"payload":{"type":"custom_tool_call","call_id":"call-1","input":"await tools.apply_patch(\"patch\");"}}"#;
        let history = parse_lines([Ok(line.to_string())]);
        assert!(history.bash_messages.is_empty());
    }

    #[test]
    fn captures_text_timestamps_for_native_history() {
        let lines = [
            r#"{"timestamp":"2026-08-03T12:00:00.125Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"before shell"}]}}"#,
        ];
        let history = parse_lines(lines.into_iter().map(|line| Ok(line.to_string())));
        assert_eq!(history.message_times["before shell"][0], 1785758400125);
    }
}
