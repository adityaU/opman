//! Unit tests for the ACP tool-call → opencode tool-part mapping.

use super::*;

fn tool_part() -> Value {
    json!({"type":"tool","id":"c1","callID":"c1","tool":"tool","messageID":"m1","sessionID":"s1",
           "state":{"status":"running","input":{},"time":{"start":1}}})
}

#[test]
fn claude_meta_tool_name_becomes_the_part_tool() {
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"_meta":{"claudeCode":{"toolName":"Bash"}}}),
    );
    assert_eq!(part["tool"], "Bash");
}

#[test]
fn task_and_agent_tool_names_map_onto_the_opencode_task_tool() {
    // opman's transcript renders subagent launches through the `task` renderer only.
    for name in ["Task", "Agent"] {
        let mut part = tool_part();
        merge(
            &mut part,
            &json!({"_meta":{"claudeCode":{"toolName":name}}}),
        );
        assert_eq!(part["tool"], "task");
    }
}

#[test]
fn kind_is_the_tool_name_fallback_but_the_other_category_is_ignored() {
    let mut part = tool_part();
    merge(&mut part, &json!({"kind":"execute"}));
    assert_eq!(part["tool"], "execute");

    // "other" carries no information, so the caller's default name must survive.
    let mut generic = tool_part();
    merge(&mut generic, &json!({"kind":"other"}));
    assert_eq!(generic["tool"], "tool");
}

/// The opening frame, as opencode sends it: the tool's own name in `title`.
fn opening(title: &str, kind: &str) -> Value {
    json!({"sessionUpdate":"tool_call","title":title,"kind":kind,"status":"pending"})
}

#[test]
fn the_opening_titles_name_becomes_the_part_tool() {
    // opencode names its tool in the opening frame and has no `_meta`, so without this
    // every one of its calls rendered as its ACP category — or as nothing, for `other`.
    for (title, kind) in [
        ("bash", "execute"),
        ("read", "read"),
        ("edit", "edit"),
        ("glob", "search"),
        ("todowrite", "other"),
        // An MCP tool arrives as `<server>_<tool>`, which is the name opman's own renderers
        // already key on — `ui_ui_render` is what routes a call to the UI renderer.
        ("ui_ui_render", "other"),
    ] {
        let mut part = tool_part();
        merge(&mut part, &opening(title, kind));
        assert_eq!(part["tool"], title, "opening title {title} should name the tool");
    }
}

#[test]
fn the_opening_title_is_not_repeated_as_the_call_subtitle() {
    // The header already prints the tool name; `state.title` is for what this call did.
    let mut part = tool_part();
    merge(&mut part, &opening("bash", "execute"));
    assert!(part["state"].get("title").is_none());

    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","title":"ls -la"}),
    );
    assert_eq!(part["state"]["title"], "ls -la");
}

#[test]
fn a_later_title_describes_the_call_and_never_renames_the_tool() {
    // Every `tool_call_update` overwrites `title` with prose about this particular call, so
    // reading any of them as a name would rename the tool mid-call.
    let mut part = tool_part();
    merge(&mut part, &opening("edit", "edit"));
    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","title":"src/main.rs","kind":"edit"}),
    );
    assert_eq!(part["tool"], "edit");
    assert_eq!(part["state"]["title"], "src/main.rs");
}

#[test]
fn a_prose_opening_title_falls_back_to_the_acp_kind() {
    // Agents that put a sentence in the opening title must not have it taken for a name.
    for title in ["Read package.json", "", "Running the test suite"] {
        let mut part = tool_part();
        merge(&mut part, &opening(title, "read"));
        assert_eq!(part["tool"], "read", "title {title:?} is not a tool name");
    }
}

#[test]
fn claude_meta_outranks_the_opening_title() {
    // Claude sends both: `_meta` is the real name, the title is a human summary.
    let mut part = tool_part();
    let mut frame = opening("Read", "read");
    frame["title"] = json!("Read package.json");
    frame["_meta"] = json!({"claudeCode":{"toolName":"Read"}});
    merge(&mut part, &frame);
    assert_eq!(part["tool"], "Read");
    assert_eq!(part["state"]["title"], "Read package.json");
}

#[test]
fn a_later_kind_does_not_displace_a_resolved_name() {
    // `kind` is a category, so it is only ever a last resort — never an overwrite.
    let mut part = tool_part();
    merge(&mut part, &opening("todowrite", "other"));
    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","kind":"think"}),
    );
    assert_eq!(part["tool"], "todowrite");
}

#[test]
fn a_later_update_still_names_a_part_that_has_no_name_yet() {
    // A client that joined mid-call creates the part from an update, so the fallback has to
    // keep working there.
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","kind":"fetch"}),
    );
    assert_eq!(part["tool"], "fetch");
}

#[test]
fn a_later_raw_input_replaces_the_earlier_partial_one() {
    // ACP streams call arguments progressively, so the newest snapshot is the complete one.
    let mut part = tool_part();
    merge(&mut part, &json!({"rawInput":{"command":"ls"}}));
    assert_eq!(part["state"]["input"], json!({"command":"ls"}));

    merge(
        &mut part,
        &json!({"rawInput":{"command":"ls -la","description":"list"}}),
    );
    assert_eq!(
        part["state"]["input"],
        json!({"command":"ls -la","description":"list"})
    );
}

#[test]
fn fields_absent_from_an_update_are_left_untouched() {
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"title":"Run tests","rawInput":{"command":"cargo test"}}),
    );
    merge(&mut part, &json!({"status":"completed"}));
    assert_eq!(part["state"]["title"], "Run tests");
    assert_eq!(part["state"]["input"], json!({"command":"cargo test"}));
    assert_eq!(part["state"]["status"], "completed");
}

#[test]
fn acp_statuses_map_onto_the_three_states_opman_renders() {
    for (acp, opman) in [
        ("pending", "running"),
        ("in_progress", "running"),
        ("completed", "completed"),
        ("failed", "error"),
    ] {
        let mut part = tool_part();
        merge(&mut part, &json!({"status":acp}));
        assert_eq!(part["state"]["status"], opman, "status {acp}");
    }
}

#[test]
fn a_failed_call_records_the_output_as_the_error_and_stamps_an_end_time() {
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"status":"failed","rawOutput":"command not found"}),
    );
    assert_eq!(part["state"]["status"], "error");
    assert_eq!(part["state"]["error"], "command not found");
    assert!(part["state"]["time"]["end"].is_number());
}

#[test]
fn a_completed_call_stamps_an_end_time() {
    let mut part = tool_part();
    merge(&mut part, &json!({"status":"completed"}));
    assert!(part["state"]["time"]["end"].is_number());
}

#[test]
fn raw_output_is_used_verbatim_when_a_string_and_serialized_when_structured() {
    let mut text = tool_part();
    merge(&mut text, &json!({"rawOutput":"hello"}));
    assert_eq!(text["state"]["output"], "hello");

    // Structured results still have to reach the UI, which only renders text.
    let mut structured = tool_part();
    merge(&mut structured, &json!({"rawOutput":{"exitCode":0}}));
    assert_eq!(structured["state"]["output"], "{\"exitCode\":0}");
}

#[test]
fn without_raw_output_the_text_content_blocks_are_joined() {
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"content":[
            {"type":"content","content":{"type":"text","text":"first"}},
            {"type":"content","content":{"type":"text","text":"second"}}
        ]}),
    );
    assert_eq!(part["state"]["output"], "first\nsecond");
}

#[test]
fn a_diff_content_block_is_carried_into_the_state_metadata() {
    let diff = json!({"type":"diff","path":"/tmp/a.rs","oldText":"a","newText":"b"});
    let mut part = tool_part();
    merge(&mut part, &json!({"content":[diff]}));
    assert_eq!(part["state"]["metadata"]["diff"], diff);
}

#[test]
fn location_paths_are_carried_into_the_state_metadata() {
    let mut part = tool_part();
    merge(
        &mut part,
        &json!({"locations":[{"path":"/tmp/a.rs"},{"path":"/tmp/b.rs","line":4}]}),
    );
    assert_eq!(
        part["state"]["metadata"]["locations"],
        json!(["/tmp/a.rs", "/tmp/b.rs"])
    );
}

#[test]
fn settle_only_moves_a_running_tool_part() {
    let mut running = tool_part();
    assert!(settle(&mut running, 99));
    assert_eq!(running["state"]["status"], "completed");
    assert_eq!(running["state"]["time"]["end"], 99);

    // Already terminal: re-emitting it would flap the UI, so nothing changes.
    let mut done = tool_part();
    done["state"]["status"] = json!("completed");
    assert!(!settle(&mut done, 99));

    let mut text = json!({"type":"text","text":"hi"});
    assert!(!settle(&mut text, 99));
}
