//! Coverage for jsonl fs-backed helpers + parser edge functions.
use super::*;
use serde_json::json;

// ---- parse_file / read_ai_title (real temp files) ---------------------

#[test]
fn parse_file_missing_returns_default() {
    let p = parse_file(std::path::Path::new("/no/such/file.jsonl"), "ses");
    assert!(p.messages.is_empty());
    assert!(p.title.is_none());
}

#[test]
fn parse_file_reads_a_real_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"type":"ai-title","aiTitle":"Titled"}"#,
            "\n",
            r#"{"type":"user","promptSource":"typed","message":{"role":"user","content":"hi"}}"#,
            "\n",
        ),
    )
    .unwrap();
    let p = parse_file(&path, "ses");
    assert_eq!(p.title.as_deref(), Some("Titled"));
    assert_eq!(p.messages.len(), 1);
}

#[test]
fn read_ai_title_variants() {
    let dir = tempfile::tempdir().unwrap();

    // No file → None.
    assert!(read_ai_title(&dir.path().join("missing.jsonl")).is_none());

    // Last non-empty ai-title wins; empty ones are ignored.
    let p1 = dir.path().join("a.jsonl");
    std::fs::write(
        &p1,
        concat!(
            r#"{"type":"ai-title","aiTitle":"First"}"#,
            "\n",
            r#"{"type":"ai-title","aiTitle":"   "}"#,
            "\n",
            r#"{"type":"ai-title","aiTitle":"Second"}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":"x"}}"#,
            "\n",
        ),
    )
    .unwrap();
    assert_eq!(read_ai_title(&p1).as_deref(), Some("Second"));

    // No ai-title line → None.
    let p2 = dir.path().join("b.jsonl");
    std::fs::write(&p2, "{\"type\":\"user\",\"message\":{\"content\":\"x\"}}\n").unwrap();
    assert!(read_ai_title(&p2).is_none());
}

// ---- read_tail ---------------------------------------------------------

#[test]
fn read_tail_missing_and_empty_return_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_tail("/no/such/output.log", 1024).is_none());
    let empty = dir.path().join("empty.log");
    std::fs::write(&empty, b"").unwrap();
    assert!(read_tail(empty.to_str().unwrap(), 1024).is_none());
}

#[test]
fn read_tail_returns_whole_small_file_and_tail_of_large() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("small.log");
    std::fs::write(&small, b"hello").unwrap();
    assert_eq!(
        read_tail(small.to_str().unwrap(), 1024).as_deref(),
        Some("hello")
    );

    let big = dir.path().join("big.log");
    std::fs::write(&big, b"0123456789").unwrap();
    // Only the last 4 bytes.
    assert_eq!(read_tail(big.to_str().unwrap(), 4).as_deref(), Some("6789"));
}

// ---- enrich_background_tasks ------------------------------------------

#[test]
fn enrich_background_tasks_tails_output_file() {
    let dir = tempfile::tempdir().unwrap();
    let out_file = dir.path().join("job.output");
    std::fs::write(&out_file, b"build log line\n").unwrap();

    let mut parsed = ParsedSession::default();
    parsed.messages.push(MsgOut {
        info: json!({"role":"assistant","id":"m1"}),
        parts: vec![json!({
            "type":"tool","tool":"Bash","id":"t1",
            "state": { "status":"running", "metadata": { "background": true, "outputFile": out_file.to_str().unwrap() } }
        })],
    });
    enrich_background_tasks(&mut parsed);
    assert_eq!(
        parsed.messages[0].parts[0]["state"]["metadata"]["output"],
        "build log line\n"
    );
}

#[test]
fn enrich_background_tasks_skips_non_bg_and_missing_file() {
    let mut parsed = ParsedSession::default();
    parsed.messages.push(MsgOut {
        info: json!({"role":"assistant","id":"m1"}),
        parts: vec![
            // non-background tool → skipped
            json!({"type":"tool","tool":"Bash","id":"t1","state":{"status":"completed"}}),
            // background but no outputFile → skipped
            json!({"type":"tool","tool":"Bash","id":"t2","state":{"metadata":{"background":true}}}),
            // background with a missing output file → read_tail None → skipped
            json!({"type":"tool","tool":"Bash","id":"t3","state":{"metadata":{"background":true,"outputFile":"/no/such.log"}}}),
        ],
    });
    enrich_background_tasks(&mut parsed);
    assert!(parsed.messages[0].parts[1]["state"]["metadata"]
        .get("output")
        .is_none());
    assert!(parsed.messages[0].parts[2]["state"]["metadata"]
        .get("output")
        .is_none());
}

// ---- has_running_background_task --------------------------------------

#[test]
fn has_running_background_task_true_and_false() {
    let mut running = ParsedSession::default();
    running.messages.push(MsgOut {
        info: json!({}),
        parts: vec![json!({"state":{"status":"running","metadata":{"background":true}}})],
    });
    assert!(has_running_background_task(&running));

    let mut done = ParsedSession::default();
    done.messages.push(MsgOut {
        info: json!({}),
        parts: vec![
            json!({"state":{"status":"completed","metadata":{"background":true}}}),
            json!({"state":{"status":"running"}}), // not a background part
        ],
    });
    assert!(!has_running_background_task(&done));
}

// ---- subagent_completed ------------------------------------------------

fn asst_with(parts: Vec<serde_json::Value>) -> MsgOut {
    MsgOut {
        info: json!({"role":"assistant"}),
        parts,
    }
}

#[test]
fn subagent_completed_states() {
    // empty transcript
    assert_eq!(subagent_completed(&ParsedSession::default()), (false, None));

    // last message is a user turn → not complete
    let mut p = ParsedSession::default();
    p.messages.push(MsgOut {
        info: json!({"role":"user"}),
        parts: vec![],
    });
    assert_eq!(subagent_completed(&p), (false, None));

    // last assistant but final part is a tool call → still running
    let mut p = ParsedSession::default();
    p.messages.push(asst_with(vec![json!({"type":"tool"})]));
    assert_eq!(subagent_completed(&p), (false, None));

    // last assistant ending in text → complete with the final text
    let mut p = ParsedSession::default();
    p.messages.push(asst_with(vec![
        json!({"type":"text","text":"final answer"}),
    ]));
    assert_eq!(
        subagent_completed(&p),
        (true, Some("final answer".to_string()))
    );

    // assistant with no parts → running
    let mut p = ParsedSession::default();
    p.messages.push(asst_with(vec![]));
    assert_eq!(subagent_completed(&p), (false, None));
}

// ---- enrich_subagents --------------------------------------------------

#[test]
fn enrich_subagents_skips_and_marks_running() {
    let mut parsed = ParsedSession::default();
    parsed.messages.push(MsgOut {
        info: json!({"role":"assistant","id":"m1"}),
        parts: vec![
            // non-task part → skipped
            json!({"type":"tool","tool":"Bash","state":{"status":"completed"}}),
            // task part without a sessionId → skipped
            json!({"type":"tool","tool":"task","state":{"status":"running","metadata":{}}}),
            // task part with a sessionId that has no on-disk transcript → running
            json!({"type":"tool","tool":"task","state":{"status":"running","metadata":{"sessionId":"nonexistent-agent-xyz-000"}}}),
        ],
    });
    enrich_subagents(&mut parsed);
    assert_eq!(parsed.messages[0].parts[2]["state"]["status"], "running");
}

// ---- parse_bg_launch edge cases ---------------------------------------

#[test]
fn parse_bg_launch_edge_cases() {
    // id marker present but no "written to:" path marker → None
    assert!(parse_bg_launch("Command running in background with ID: abc123").is_none());
    // id marker immediately followed by a non-id char → empty id → None
    assert!(parse_bg_launch("background with ID: . written to: /x").is_none());
}

// ---- parse_task_notification edge cases -------------------------------

#[test]
fn parse_task_notification_edge_cases() {
    // not a task-notification block
    assert!(parse_task_notification("plain text").is_none());
    // missing task-id tag
    assert!(
        parse_task_notification("<task-notification><status>ok</status></task-notification>")
            .is_none()
    );
    // empty task-id
    assert!(
        parse_task_notification("<task-notification><task-id></task-id></task-notification>")
            .is_none()
    );

    // failed status, no summary → summary None
    let n = parse_task_notification(
        "<task-notification><task-id>abc</task-id><status>FAILED</status></task-notification>",
    )
    .unwrap();
    assert_eq!(n.task_id, "abc");
    assert!(n.failed);
    assert!(n.summary.is_none());

    // empty summary is treated as None
    let n = parse_task_notification(
        "<task-notification><task-id>abc</task-id><status>completed</status><summary></summary></task-notification>",
    )
    .unwrap();
    assert!(!n.failed);
    assert!(n.summary.is_none());
}

#[test]
fn parse_agent_id_empty_after_marker() {
    // marker present but followed immediately by a non-id char → None
    assert!(parse_agent_id("agentId: (nothing)").is_none());
}

// ---- matched background notification without summary ------------------

#[test]
fn matched_bg_notification_without_summary_completes_part() {
    let transcript = concat!(
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"x","run_in_background":true}}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"Command running in background with ID: jobA. Output is being written to: /tmp/jobA.output. You will be notified when it completes."}]}}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:05:00.000Z","message":{"role":"user","content":"<task-notification>\n<task-id>jobA</task-id>\n<status>completed</status>\n</task-notification>"}}"#,
        "\n",
    );
    let p = parse_str(transcript, "ses");
    assert_eq!(
        p.messages.len(),
        1,
        "notification folds into the part, no bubble"
    );
    let s = &p.messages[0].parts[0]["state"];
    assert_eq!(s["status"], "completed");
    assert!(s["metadata"].get("summary").is_none());
    assert!(s["time"]["end"].is_u64());
}
