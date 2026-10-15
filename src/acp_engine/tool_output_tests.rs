//! Output from recorded `codex-acp` frames: terminal chunks under `_meta`, exit statuses in
//! `rawOutput`, and MCP results.

use serde_json::{json, Value};

use super::super::tool::merge;
use super::raw_output;

fn bash_part() -> Value {
    json!({"type":"tool","id":"c1","callID":"c1","tool":"bash","messageID":"m1","sessionID":"s1",
           "state":{"status":"running","input":{"command":"x"},"time":{"start":1}}})
}

fn chunk(data: &str) -> Value {
    json!({"sessionUpdate":"tool_call_update","toolCallId":"e1",
        "_meta":{"terminal_output_delta":{"data":data,"terminal_id":"e1"}}})
}

#[test]
fn terminal_chunks_stream_live_then_become_the_output() {
    let mut part = bash_part();
    merge(&mut part, &chunk("line 1\n"));
    merge(&mut part, &chunk("line 2\n"));
    assert_eq!(part["state"]["metadata"]["output"], "line 1\nline 2\n");
    assert!(part["state"].get("output").is_none());

    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","status":"completed",
            "_meta":{"terminal_exit":{"exit_code":0,"signal":null,"terminal_id":"e1"}}}),
    );
    assert_eq!(part["state"]["output"], "line 1\nline 2\n");
    // Moved, not copied: the live buffer is not kept a second time.
    assert!(part["state"]["metadata"].get("output").is_none());
    assert_eq!(part["state"]["metadata"]["exit"], 0);
}

#[test]
fn a_failed_command_reports_its_exit_code_and_keeps_its_output() {
    let mut part = bash_part();
    merge(&mut part, &chunk("ZeroDivisionError: division by zero\n"));
    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","status":"failed",
            "_meta":{"terminal_exit":{"exit_code":1,"signal":null,"terminal_id":"e1"}}}),
    );
    assert_eq!(part["state"]["status"], "error");
    assert_eq!(part["state"]["error"], "Exit code 1");
    assert_eq!(
        part["state"]["output"],
        "ZeroDivisionError: division by zero\n"
    );
}

#[test]
fn an_exit_status_is_not_printed_as_output() {
    // A read or search command ends with `{"exit_code": 0}` and its output as one chunk.
    let mut part = bash_part();
    merge(
        &mut part,
        &json!({"sessionUpdate":"tool_call_update","status":"completed","rawOutput":{"exit_code":0},
            "_meta":{"terminal_output_delta":{"data":"hello\nthere\n","terminal_id":"e1"}}}),
    );
    assert_eq!(part["state"]["output"], "hello\nthere\n");
    assert_eq!(part["state"]["metadata"]["exit"], 0);
}

#[test]
fn a_snapshot_replaces_the_live_output() {
    let mut part = bash_part();
    merge(&mut part, &chunk("partial"));
    merge(
        &mut part,
        &json!({"_meta":{"terminal_output":{"data":"whole","terminal_id":"e1"}}}),
    );
    assert_eq!(part["state"]["metadata"]["output"], "whole");
}

#[test]
fn a_final_output_from_the_agent_wins_over_the_streamed_copy() {
    let mut part = bash_part();
    merge(&mut part, &chunk("streamed"));
    merge(&mut part, &json!({"status":"completed","rawOutput":"final"}));
    assert_eq!(part["state"]["output"], "final");
    assert!(part["state"]["metadata"].get("output").is_none());
}

#[test]
fn an_mcp_result_is_unwrapped_to_its_text() {
    let ok = json!({"rawOutput":{"result":{"content":[{"type":"text","text":"a"},
        {"type":"text","text":"b"}]},"error":null}});
    assert_eq!(raw_output(&ok).as_deref(), Some("a\nb"));

    let failed = json!({"rawOutput":{"result":null,"error":{"message":"no such tool"}}});
    assert_eq!(raw_output(&failed).as_deref(), Some("no such tool"));
}
