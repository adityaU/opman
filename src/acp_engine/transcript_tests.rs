//! Transcript folding: the streaming path that makes text appear per token.

use super::*;
use crate::acp_engine::attach::Prompt;

fn text_of(t: &Transcript, msg: usize, part: usize) -> String {
    t.messages()[msg].parts[part]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Consecutive chunks must accumulate into one part and emit *deltas*, not the whole
/// accumulated string. Re-sending everything per token is what made the old engine feel
/// like it arrived all at once.
#[test]
fn consecutive_chunks_append_and_emit_deltas() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "he", &mut out);
    t.chunk(Chunk::Text, Some("m1"), "llo", &mut out);

    assert_eq!(text_of(&t, 0, 0), "hello");
    assert_eq!(t.messages()[0].parts.len(), 1);
    // message envelope, first part, then a delta for the second chunk.
    assert!(matches!(out[0], Emit::Message(_)));
    assert!(matches!(out[1], Emit::Part(_)));
    match &out[2] {
        Emit::Delta { delta, part_id, .. } => {
            assert_eq!(delta, "llo");
            assert_eq!(part_id, "m1:0");
        }
        other => panic!("expected a delta, got {other:?}"),
    }
}

/// Reasoning and text are different part types, so a switch must open a new part rather
/// than appending thinking into the visible reply.
#[test]
fn switching_chunk_kind_opens_a_new_part() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Reasoning, Some("m1"), "thinking", &mut out);
    t.chunk(Chunk::Text, Some("m1"), "answer", &mut out);

    let parts = &t.messages()[0].parts;
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["type"], "reasoning");
    assert_eq!(parts[1]["type"], "text");
}

/// Text resumed after a tool call is a separate part; appending to the pre-tool part would
/// render the reply out of order.
#[test]
fn text_after_a_tool_call_starts_a_new_part() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "before", &mut out);
    t.tool_upsert(
        &json!({ "toolCallId": "c1", "title": "Terminal", "status": "pending" }),
        &mut out,
    );
    t.chunk(Chunk::Text, Some("m1"), "after", &mut out);

    let parts = &t.messages()[0].parts;
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0]["text"], "before");
    assert_eq!(parts[1]["type"], "tool");
    assert_eq!(parts[2]["text"], "after");
}

/// A new agent message id closes the previous message instead of merging the two.
#[test]
fn a_new_message_id_completes_the_previous_message() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "first", &mut out);
    t.chunk(Chunk::Text, Some("m2"), "second", &mut out);

    assert_eq!(t.messages().len(), 2);
    assert!(t.messages()[0].info["time"]["completed"].is_u64());
}

/// Agents that send no message id still get one message per turn, not one per chunk.
#[test]
fn chunks_without_a_message_id_share_one_message() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, None, "a", &mut out);
    t.chunk(Chunk::Text, None, "b", &mut out);

    assert_eq!(t.messages().len(), 1);
    assert_eq!(text_of(&t, 0, 0), "ab");
}

/// A user prompt ends the streaming message, so the next reply is not appended to the last.
#[test]
fn a_user_message_closes_the_live_assistant_message() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, None, "reply", &mut out);
    t.user_message(&Prompt::text("next question"), &mut out);
    t.chunk(Chunk::Text, None, "second reply", &mut out);

    assert_eq!(t.messages().len(), 3);
    assert_eq!(t.messages()[1].info["role"], "user");
    assert_eq!(t.messages()[2].info["role"], "assistant");
}

/// An interrupted turn must not leave a tool spinning forever in the UI.
#[test]
fn finishing_a_turn_settles_tools_left_running() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.tool_upsert(
        &json!({ "toolCallId": "c1", "title": "Terminal", "status": "in_progress" }),
        &mut out,
    );
    t.finish_turn(&mut out);

    let part = &t.messages()[0].parts[0];
    assert_eq!(part["state"]["status"], "completed");
    assert!(part["state"]["time"]["end"].is_u64());
}

/// Usage lands on the live assistant envelope, which is how tokens and cost become visible.
#[test]
fn usage_updates_the_live_message_envelope() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "hi", &mut out);
    t.set_usage(json!({ "input": 7 }), Some(0.25), &mut out);

    assert_eq!(t.messages()[0].info["tokens"]["input"], 7);
    assert_eq!(t.messages()[0].info["cost"], 0.25);
}

/// Reload hands history back to the agent so a `session/load` replay rebuilds it rather
/// than doubling it — while keeping the session's identity and chosen model.
#[test]
fn begin_replay_drops_history_but_keeps_identity() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.set_model("opus");
    t.chunk(Chunk::Text, Some("m1"), "old", &mut out);
    t.begin_replay();
    out.clear();
    t.chunk(Chunk::Text, Some("m2"), "new", &mut out);

    assert_eq!(t.messages().len(), 1);
    assert_eq!(t.messages()[0].info["sessionID"], "ses1");
    assert_eq!(t.messages()[0].info["model"], "opus");
}

/// The prompt that triggered the connection is rendered before `session/load` runs, and
/// the agent has never seen it — so the replay cannot contain it. Clearing it outright is
/// what made a first message after a restart disappear from the transcript.
#[test]
fn a_replay_does_not_swallow_the_prompt_that_triggered_it() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    let pending = t.user_message(&Prompt::text("what changed?"), &mut out);

    t.begin_replay();
    assert!(t.messages().is_empty(), "history is the agent's to rebuild");
    t.user_message(&Prompt::text("earlier question"), &mut out);
    t.chunk(Chunk::Text, Some("m1"), "earlier answer", &mut out);
    t.end_replay(&mut out);

    let ids: Vec<&str> = t
        .messages()
        .iter()
        .filter_map(|m| m.info["id"].as_str())
        .collect();
    assert_eq!(ids.len(), 3, "replayed history plus the held prompt");
    assert_eq!(ids[2], pending, "the unsent prompt lands last, in time order");
    // Ids stay unique across the reset: the client already rendered the held prompt under
    // its own id, so a replayed message must not claim the same one.
    assert_ne!(ids[0], pending);
}

/// A replayed conversation is finished business. Leaving the last assistant message open
/// would render an old session as though it were still streaming.
#[test]
fn end_replay_settles_the_last_replayed_message() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "done", &mut out);
    t.end_replay(&mut out);

    assert!(t.messages()[0].info["time"]["completed"].is_u64());
}

/// Every replayed turn is settled, not just the last. The web UI scans back for the newest
/// unfinished assistant message and calls every user prompt after it "queued" — so one
/// unstamped message mid-history mislabels the rest of the conversation.
#[test]
fn a_replay_settles_every_turn_it_rebuilds() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.begin_replay();
    t.user_message(&Prompt::text("first"), &mut out);
    t.chunk(Chunk::Text, Some("m1"), "first answer", &mut out);
    t.user_message(&Prompt::text("second"), &mut out);
    t.chunk(Chunk::Text, Some("m2"), "second answer", &mut out);
    t.end_replay(&mut out);

    let unfinished = t
        .messages()
        .iter()
        .filter(|m| m.info["role"] == "assistant")
        .filter(|m| !m.info["time"]["completed"].is_u64())
        .count();
    assert_eq!(unfinished, 0);
}

/// Live, a follow-up sent mid-turn must not settle the turn it interrupts: an agent that
/// advertises steering is still generating, and its running tools are still running.
#[test]
fn a_live_follow_up_does_not_settle_the_turn_it_interrupts() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "thinking", &mut out);
    t.user_message(&Prompt::text("actually, wait"), &mut out);

    assert!(!t.messages()[0].info["time"]["completed"].is_u64());
}
