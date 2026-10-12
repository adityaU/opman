//! Which model a reply is stamped with: the one the agent reports, not the one asked for.

use super::*;

/// The reported bug: Sonnet was chosen, the agent ran Fable, and the reply still said Sonnet.
#[test]
fn the_model_the_agent_ran_on_replaces_the_requested_one() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.set_model("sonnet");
    t.chunk(Chunk::Text, Some("m1"), "hi", &mut out);
    assert_eq!(t.messages()[0].info["model"], "sonnet");

    out.clear();
    t.note_ran_on("claude-fable-5-1", &mut out);
    assert_eq!(t.messages()[0].info["model"], "claude-fable-5-1");
    assert!(matches!(out.as_slice(), [Emit::Message(_)]));
}

#[test]
fn a_repeated_report_emits_nothing() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.chunk(Chunk::Text, Some("m1"), "hi", &mut out);
    t.note_ran_on("claude-sonnet-5-5", &mut out);
    out.clear();
    t.note_ran_on("claude-sonnet-5-5", &mut out);
    assert!(out.is_empty());
}

#[test]
fn a_report_with_no_live_message_is_ignored() {
    let mut t = Transcript::new("ses1");
    let mut out = Vec::new();
    t.note_ran_on("claude-sonnet-5-5", &mut out);
    assert!(out.is_empty());
    assert!(t.messages().is_empty());
}
