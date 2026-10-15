//! Codex names no tools: each call is a category, a prose title, locations and diffs. These
//! frames are copied from a recorded `codex-acp` 2.2.2 turn, so the cards opman picks for
//! them are pinned to what the adapter really sends.

use serde_json::{json, Value};

use super::super::tool::merge;

fn tool_part() -> Value {
    json!({"type":"tool","id":"c1","callID":"c1","tool":"tool","messageID":"m1","sessionID":"s1",
           "state":{"status":"running","input":{},"time":{"start":1}}})
}

fn merged(frames: &[Value]) -> Value {
    let mut part = tool_part();
    for frame in frames {
        merge(&mut part, frame);
    }
    part
}

#[test]
fn a_codex_command_renders_as_bash_even_when_its_title_looks_like_a_name() {
    // `pwd` passes the identifier check, but it is the command, not the tool.
    let part = merged(&[json!({"sessionUpdate":"tool_call","name":"exec_command","kind":"execute",
        "title":"pwd","status":"in_progress",
        "content":[{"type":"terminal","terminalId":"exec-1"}],
        "rawInput":{"command":"pwd","cwd":"/tmp/p"}})]);
    assert_eq!(part["tool"], "bash");
    assert_eq!(part["state"]["input"]["command"], "pwd");
    assert_eq!(part["state"]["title"], "pwd");
}

#[test]
fn a_codex_file_read_renders_as_read_with_the_location_as_its_path() {
    let part = merged(&[json!({"sessionUpdate":"tool_call","name":"exec_command","kind":"read",
        "title":"Read file '/tmp/p/a.txt'","status":"in_progress",
        "locations":[{"path":"/tmp/p/a.txt"}]})]);
    assert_eq!(part["tool"], "read");
    assert_eq!(part["state"]["input"]["file_path"], "/tmp/p/a.txt");
}

#[test]
fn a_read_with_no_file_is_a_listing() {
    let part = merged(&[json!({"sessionUpdate":"tool_call","name":"exec_command","kind":"read",
        "title":"List files","status":"in_progress"})]);
    assert_eq!(part["tool"], "ls");
}

#[test]
fn a_codex_search_renders_as_grep_and_a_web_search_as_websearch() {
    let grep = merged(&[json!({"sessionUpdate":"tool_call","kind":"search",
        "title":"Search for 'there' in .","status":"in_progress"})]);
    assert_eq!(grep["tool"], "grep");

    let web = merged(&[
        json!({"sessionUpdate":"tool_call","kind":"search","title":"Web search","status":"in_progress",
            "rawInput":{"type":"webSearch","id":"w1","query":"","action":null}}),
        json!({"sessionUpdate":"tool_call_update","title":"Web search: acp","status":"completed",
            "rawInput":{"type":"webSearch","id":"w1","query":"acp",
                "action":{"type":"search","query":"acp","queries":null}}}),
    ]);
    assert_eq!(web["tool"], "websearch");
    assert_eq!(web["state"]["input"]["query"], "acp");
}

#[test]
fn a_codex_patch_on_an_existing_file_becomes_edit_arguments() {
    let part = merged(&[json!({"sessionUpdate":"tool_call","kind":"edit","title":"Editing files",
        "status":"in_progress","content":[{"type":"diff","oldText":"hello\nworld\n",
        "newText":"hello\nthere\n","path":"/tmp/p/a.txt","_meta":{"kind":"update"}}]})]);
    assert_eq!(part["tool"], "edit");
    assert_eq!(
        part["state"]["input"],
        json!({"file_path":"/tmp/p/a.txt","old_string":"hello\nworld\n","new_string":"hello\nthere\n"})
    );
}

#[test]
fn a_codex_patch_that_adds_a_file_becomes_write_arguments() {
    let part = merged(&[json!({"sessionUpdate":"tool_call","kind":"edit","title":"Editing files",
        "status":"in_progress","content":[{"type":"diff","oldText":null,"newText":"hi\n",
        "path":"/tmp/p/b.txt","_meta":{"kind":"add"}}]})]);
    assert_eq!(part["tool"], "write");
    assert_eq!(
        part["state"]["input"],
        json!({"file_path":"/tmp/p/b.txt","content":"hi\n"})
    );
}

#[test]
fn a_patch_with_several_hunks_becomes_multi_edit_arguments() {
    let diff = |old: &str, new: &str| json!({"type":"diff","oldText":old,"newText":new,"path":"/a.rs"});
    let part = merged(&[json!({"sessionUpdate":"tool_call","kind":"edit","title":"Editing files",
        "content":[diff("a","b"), diff("c","d")]})]);
    assert_eq!(part["tool"], "edit");
    assert_eq!(part["state"]["input"]["file_path"], "/a.rs");
    assert_eq!(part["state"]["input"]["edits"][1]["new_string"], "d");
}

#[test]
fn a_native_edit_input_is_never_overwritten_by_its_diff() {
    // Claude names its tool and sends native arguments alongside the ACP diff.
    let part = merged(&[json!({"sessionUpdate":"tool_call","kind":"edit","title":"Edit a.rs",
        "_meta":{"claudeCode":{"toolName":"Edit"}},
        "rawInput":{"file_path":"/a.rs","old_string":"x","new_string":"y"},
        "content":[{"type":"diff","oldText":"q","newText":"r","path":"/a.rs"}]})]);
    assert_eq!(part["tool"], "Edit");
    assert_eq!(part["state"]["input"]["old_string"], "x");
}

#[test]
fn a_codex_mcp_call_takes_the_mcp_name_and_its_arguments_as_input() {
    let input = json!({"server":"ui","tool":"ui_render","arguments":{"title":"T","blocks":[]}});
    let part = merged(&[
        json!({"sessionUpdate":"tool_call","kind":"execute","title":"mcp.ui.ui_render",
            "status":"in_progress","rawInput":input,"_meta":{"is_mcp_tool_call":true}}),
        // The completion repeats the wrapped input without the `_meta` flag.
        json!({"sessionUpdate":"tool_call_update","status":"completed","rawInput":input}),
    ]);
    assert_eq!(part["tool"], "mcp__ui__ui_render");
    assert_eq!(part["state"]["input"], json!({"title":"T","blocks":[]}));
}
