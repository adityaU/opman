use super::*;

use crate::acp_engine::config::AgentConfig;

fn engine() -> Arc<AcpEngine> {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    Arc::new(AcpEngine::new(
        "test".to_string(),
        AgentConfig::default(),
        None,
        registry,
    ))
}

fn allow_options() -> Vec<Value> {
    vec![
        json!({ "optionId": "a", "name": "Postgres", "kind": "allow_once" }),
        json!({ "optionId": "b", "name": "SQLite", "kind": "allow_once" }),
    ]
}

// ── detection ───────────────────────────────────────────────────────

#[test]
fn a_questions_array_is_a_question_whatever_the_tool_is_called() {
    let call = json!({
        "rawInput": { "questions": [{
            "question": "Which database?",
            "header": "Database",
            "options": [{ "label": "Postgres", "description": "Managed" }],
            "multiSelect": true,
        }] }
    });
    let questions = from_tool_call("some_unknown_tool", &call).expect("recognised");
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["question"], "Which database?");
    assert_eq!(questions[0]["header"], "Database");
    assert_eq!(questions[0]["multiple"], true);
    assert_eq!(questions[0]["options"][0]["label"], "Postgres");
    // Free text is always allowed: the card must not force the agent's shortlist.
    assert_eq!(questions[0]["custom"], true);
}

#[test]
fn a_named_tool_reads_its_whole_input_as_one_question() {
    let call = json!({
        "rawInput": {
            "question": "Rewrite or migrate?",
            "follow_up": ["Rewrite", "Migrate"],
        }
    });
    let questions = from_tool_call("ask_followup_question", &call).expect("recognised");
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["question"], "Rewrite or migrate?");
    // Bare strings are options too.
    assert_eq!(questions[0]["options"][1]["label"], "Migrate");
    // No header supplied, but a tab still has to be labelled.
    assert_eq!(questions[0]["header"], "Question");
    assert_eq!(questions[0]["multiple"], false);
}

#[test]
fn an_ordinary_permission_request_is_left_alone() {
    let write = json!({ "rawInput": { "file_path": "/tmp/x", "content": "hi" } });
    assert!(from_tool_call("Write", &write).is_none());
    // A recognised name with nothing to render has no card to show.
    assert!(from_tool_call("AskUserQuestion", &json!({ "rawInput": {} })).is_none());
    // An empty list is not a question either.
    let empty = json!({ "rawInput": { "questions": [] } });
    assert!(from_tool_call("AskUserQuestion", &empty).is_none());
    // No rawInput at all.
    assert!(from_tool_call("AskUserQuestion", &json!({ "title": "Ask" })).is_none());
}

#[test]
fn the_alternate_multi_select_spelling_is_honoured() {
    let call = json!({ "rawInput": { "questions": [{ "question": "Pick", "multiple": true }] } });
    let questions = from_tool_call("x", &call).expect("recognised");
    assert_eq!(questions[0]["multiple"], true);
}

// ── the round trip ──────────────────────────────────────────────────

#[tokio::test]
async fn an_answer_selects_the_option_it_names() {
    let engine = engine();
    let mut events = engine.subscribe_raw();
    let questions = vec![json!({ "question": "Which database?" })];

    let asking = tokio::spawn({
        let engine = engine.clone();
        let options = allow_options();
        async move { ask(&engine, "ses-1", "/repo", questions, &options).await }
    });

    let asked = next_event(&mut events, "question.asked").await;
    let id = asked["properties"]["id"].as_str().expect("id").to_string();
    assert_eq!(asked["properties"]["sessionID"], "ses-1");

    assert!(engine.resolve_pending(
        &id,
        PendingReply::Question(vec![vec!["SQLite".to_string()]])
    ));
    let outcome = asking.await.expect("ask completes");
    assert_eq!(outcome["outcome"]["outcome"], "selected");
    assert_eq!(outcome["outcome"]["optionId"], "b");
    assert_eq!(outcome["_meta"]["opman"]["answers"][0][0], "SQLite");
}

#[tokio::test]
async fn free_text_the_agent_never_offered_cancels_rather_than_guessing() {
    let engine = engine();
    let mut events = engine.subscribe_raw();
    let questions = vec![json!({ "question": "Which database?" })];

    let asking = tokio::spawn({
        let engine = engine.clone();
        let options = allow_options();
        async move { ask(&engine, "ses-1", "/repo", questions, &options).await }
    });
    let asked = next_event(&mut events, "question.asked").await;
    let id = asked["properties"]["id"].as_str().expect("id").to_string();

    engine.resolve_pending(&id, PendingReply::Question(vec![vec!["DuckDB".into()]]));
    let outcome = asking.await.expect("ask completes");
    assert_eq!(outcome["outcome"]["outcome"], "cancelled");
    // The words still travel, so an adapter that looks can use them.
    assert_eq!(outcome["_meta"]["opman"]["answers"][0][0], "DuckDB");
}

#[tokio::test]
async fn a_dismissed_question_cancels_and_clears_its_card() {
    let engine = engine();
    let mut events = engine.subscribe_raw();
    let questions = vec![json!({ "question": "Which database?" })];

    let asking = tokio::spawn({
        let engine = engine.clone();
        let options = allow_options();
        async move { ask(&engine, "ses-1", "/repo", questions, &options).await }
    });
    let asked = next_event(&mut events, "question.asked").await;
    let id = asked["properties"]["id"].as_str().expect("id").to_string();

    assert!(engine.resolve_pending(&id, PendingReply::Reject));
    let outcome = asking.await.expect("ask completes");
    assert_eq!(outcome["outcome"]["outcome"], "cancelled");

    // The clearing event is what stops the card coming back on the next reconnect.
    let cleared = next_event(&mut events, "question.replied").await;
    assert_eq!(cleared["properties"]["requestID"], id.as_str());
}

#[test]
fn an_answer_may_name_an_option_by_id() {
    assert_eq!(
        option_for_answer(&allow_options(), "b").as_deref(),
        Some("b")
    );
    // Matching by name is case-insensitive, since the label round-trips through the UI.
    assert_eq!(
        option_for_answer(&allow_options(), "postgres").as_deref(),
        Some("a")
    );
    assert!(option_for_answer(&allow_options(), "MySQL").is_none());
}

async fn next_event(events: &mut tokio::sync::broadcast::Receiver<String>, wanted: &str) -> Value {
    loop {
        let raw = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("an event arrives")
            .expect("the channel stays open");
        let event: Value = serde_json::from_str(&raw).expect("valid event json");
        if event["type"] == wanted {
            return event;
        }
    }
}
