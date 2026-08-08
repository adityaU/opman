//! Question-shaped permission requests.
//!
//! ACP has no ask-the-user primitive: `session/request_permission` is an allow/reject gate.
//! An adapter that wants to expose its harness's question tool therefore has only one place
//! to put it — a permission request whose `rawInput` carries the questions and whose
//! `options` are the answers. When one arrives, showing it as a permission card would ask
//! the user to "allow" a question, which is not a thing they can meaningfully answer.
//!
//! This module recognises that shape and renders the real choice instead. The answer is
//! matched back onto the option the agent offered, by id and then by label.
//!
//! It is deliberately the *secondary* path. The primary one is the `ask` MCP server, which
//! works whatever the agent does, because a permission response has nowhere to put an
//! answer the agent did not already enumerate as an option.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::AcpEngine;
use crate::claude_engine::PendingReply;

/// How long a question waits for a human. Same ceiling as a permission request.
const QUESTION_TIMEOUT: Duration = Duration::from_secs(3600);

/// Tool names that mean "ask the user", across the harnesses opman has seen.
const QUESTION_TOOLS: [&str; 4] = [
    "AskUserQuestion",
    "ask_followup_question",
    "ask_user_question",
    "ask_user",
];

/// The questions in a tool call, when it is one. `None` means an ordinary permission
/// request, which the caller handles as before.
pub(super) fn from_tool_call(tool: &str, tool_call: &Value) -> Option<Vec<Value>> {
    let raw = tool_call.get("rawInput")?;
    // A `questions` array is self-describing, so it is trusted whatever the tool is called.
    let listed = raw.get("questions").and_then(Value::as_array);
    if let Some(questions) = listed.filter(|questions| !questions.is_empty()) {
        return Some(questions.iter().map(normalise).collect());
    }
    // A recognised tool name earns the single-question reading, where the whole input is
    // the question — the shape `ask_followup_question` uses.
    let named = QUESTION_TOOLS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(tool));
    if !named || str_at(raw, "question").is_empty() {
        return None;
    }
    Some(vec![normalise(raw)])
}

/// One question in opman's `QuestionRequest` shape. Harnesses disagree on where the
/// options live and how the multi-select flag is spelled, and an absent `header` still has
/// to label a tab.
fn normalise(question: &Value) -> Value {
    let text = str_at(question, "question");
    let header = match str_at(question, "header") {
        header if !header.is_empty() => header,
        _ => "Question".to_string(),
    };
    let options: Vec<Value> = ["options", "follow_up", "suggestions"]
        .iter()
        .find_map(|key| question.get(*key).and_then(Value::as_array))
        .map(|options| options.iter().map(normalise_option).collect())
        .unwrap_or_default();
    let multiple = ["multiSelect", "multiple"]
        .iter()
        .any(|key| question.get(*key).and_then(Value::as_bool).unwrap_or(false));
    json!({
        "question": text,
        "header": header,
        "options": options,
        "multiple": multiple,
        "custom": true,
    })
}

/// An option may be a bare string or an object; both become `{label, description}`.
fn normalise_option(option: &Value) -> Value {
    if let Some(label) = option.as_str() {
        return json!({ "label": label, "description": "" });
    }
    json!({
        "label": str_at(option, "label"),
        "description": str_at(option, "description"),
    })
}

fn str_at(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Show the card, wait, and answer the agent with the option its answer names.
pub(super) async fn ask(
    engine: &Arc<AcpEngine>,
    session_id: &str,
    dir: &str,
    questions: Vec<Value>,
    options: &[Value],
) -> Value {
    let request_id = super::rand_id("qst");
    engine.emit(
        dir,
        "question.asked",
        json!({ "id": request_id, "sessionID": session_id, "questions": questions }),
    );

    let rx = engine.register_pending(&request_id, session_id);
    let answers = match tokio::time::timeout(QUESTION_TIMEOUT, rx).await {
        Ok(Ok(PendingReply::Question(answers))) => answers,
        _ => {
            engine.resolve_pending(&request_id, PendingReply::Reject);
            Vec::new()
        }
    };
    // Clear the card on every path, including the ones no reply route sees — a timed-out
    // question left on screen invites the user to answer a turn that already unwound.
    engine.emit(
        dir,
        "question.replied",
        json!({ "id": request_id, "requestID": request_id, "sessionID": session_id }),
    );

    match answers.first().and_then(|picked| picked.first()) {
        Some(answer) => match option_for_answer(options, answer) {
            Some(id) => selected(&id, &answers),
            // The user typed something the agent never offered. Cancelling is the only
            // truthful outcome: there is no option that carries their words.
            None => cancelled(&answers),
        },
        None => cancelled(&answers),
    }
}

/// The option an answer names: by `optionId` first, then by a case-insensitive `name`.
fn option_for_answer(options: &[Value], answer: &str) -> Option<String> {
    let matches = |option: &&Value| {
        let id = option.get("optionId").and_then(Value::as_str);
        let name = option.get("name").and_then(Value::as_str);
        id.is_some_and(|id| id == answer) || name.is_some_and(|n| n.eq_ignore_ascii_case(answer))
    };
    options
        .iter()
        .find(matches)
        .and_then(|option| option.get("optionId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The answers ride in `_meta`: ACP's response has no field for them, and an adapter that
/// bridged its question tool this way is the one place that would know to look.
fn selected(option_id: &str, answers: &[Vec<String>]) -> Value {
    json!({
        "outcome": { "outcome": "selected", "optionId": option_id },
        "_meta": { "opman": { "answers": answers } },
    })
}

fn cancelled(answers: &[Vec<String>]) -> Value {
    json!({
        "outcome": { "outcome": "cancelled" },
        "_meta": { "opman": { "answers": answers } },
    })
}

#[cfg(test)]
#[path = "question_tests.rs"]
mod question_tests;
