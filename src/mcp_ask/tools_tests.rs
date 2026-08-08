use super::*;

fn question(text: &str, options: usize) -> Value {
    let options: Vec<Value> = (0..options)
        .map(|i| json!({ "label": format!("option {i}"), "description": "" }))
        .collect();
    json!({ "question": text, "header": "H", "options": options })
}

fn call(questions: Value) -> Value {
    json!({ "name": TOOL_NAME, "arguments": { "questions": questions } })
}

// ── schema ──────────────────────────────────────────────────────────

#[test]
fn the_tool_advertises_one_named_entry_with_a_schema() {
    let defs = definitions();
    let tools = defs.as_array().expect("an array of tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], TOOL_NAME);
    assert_eq!(tools[0]["inputSchema"]["required"][0], "questions");
    assert!(
        tools[0]["description"]
            .as_str()
            .is_some_and(|d| d.contains("block until they answer")),
        "the description has to say the call waits"
    );
}

// ── argument validation ─────────────────────────────────────────────

#[test]
fn a_well_formed_call_is_accepted() {
    let params = call(json!([question("Which database?", 2)]));
    let parsed = questions(Some(&params)).expect("accepted");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["question"], "Which database?");
}

#[test]
fn a_call_with_nothing_to_ask_is_rejected_without_a_round_trip() {
    for params in [json!({}), call(json!([])), call(json!("not an array"))] {
        let error = questions(Some(&params)).expect_err("rejected");
        assert!(error.contains("non-empty array"), "got: {error}");
    }
    assert!(questions(None).is_err());
}

#[test]
fn more_questions_than_a_card_can_carry_are_rejected() {
    let many: Vec<Value> = (0..5).map(|i| question(&format!("q{i}"), 2)).collect();
    let error = questions(Some(&call(json!(many)))).expect_err("rejected");
    assert!(error.contains("at most 4"), "got: {error}");
}

#[test]
fn a_question_the_user_could_not_act_on_is_rejected() {
    // One option is not a choice.
    let error = questions(Some(&call(json!([question("Pick", 1)])))).expect_err("rejected");
    assert!(error.contains("Question 1"), "got: {error}");

    // Blank question text.
    let blank = json!([{ "question": "  ", "header": "H", "options": [
        { "label": "a" }, { "label": "b" }
    ]}]);
    assert!(questions(Some(&call(blank))).is_err());

    // Options present but unlabelled.
    let unlabelled = json!([{ "question": "Pick", "header": "H", "options": [{}, {}] }]);
    assert!(questions(Some(&call(unlabelled))).is_err());

    // The index in the complaint points at the offending question, not the first.
    let mixed = json!([question("ok", 2), question("bad", 0)]);
    let error = questions(Some(&call(mixed))).expect_err("rejected");
    assert!(error.contains("Question 2"), "got: {error}");
}

// ── answers ─────────────────────────────────────────────────────────

#[tokio::test]
async fn with_no_loopback_the_agent_is_told_to_choose_a_default() {
    let text = ask(None, None, "/repo", vec![question("Pick", 2)]).await;
    assert!(text.contains("not running"), "got: {text}");
    assert!(text.contains("default"), "got: {text}");
}

#[test]
fn answers_are_paired_back_with_the_questions_they_answer() {
    let asked = vec![question("Which database?", 2), question("Deploy now?", 2)];
    let answers = vec![json!(["Postgres", "SQLite"]), json!(["No"])];
    let text = format_answers(&asked, &answers);
    assert!(
        text.contains("Which database? → Postgres, SQLite"),
        "got: {text}"
    );
    assert!(text.contains("Deploy now? → No"), "got: {text}");
    assert!(
        text.contains("Do not ask the same question again"),
        "got: {text}"
    );
}

#[test]
fn a_question_left_blank_reads_as_unanswered_rather_than_as_a_choice() {
    let asked = vec![question("Which database?", 2), question("Deploy now?", 2)];
    let text = format_answers(&asked, &[json!(["Postgres"])]);
    assert!(text.contains("Deploy now? → (no answer)"), "got: {text}");
}

#[test]
fn an_empty_selection_is_not_a_selection() {
    assert!(!has_selection(&json!([])));
    assert!(!has_selection(&json!([""])));
    assert!(!has_selection(&json!("not an array")));
    assert!(has_selection(&json!(["Postgres"])));
}
