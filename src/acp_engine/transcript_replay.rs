//! The history half of [`Transcript`]: user prompts, and the `session/load` replay.
//!
//! Split from [`super::transcript`], which folds the live update stream. The two pull on the
//! same state but answer different questions — what the agent is saying now, versus what the
//! conversation already was — and a replay is the one moment they conflict, because the agent
//! re-sends messages opman has already rendered.

use serde_json::json;

use super::attach::Prompt;
use super::emit::Emit;
use super::transcript::Transcript;
use crate::claude_engine::jsonl::MsgOut;

impl Transcript {
    /// Hand history back to the agent: `session/load` is about to replay the conversation,
    /// so what is rendered now would double.
    ///
    /// The trailing user messages are held rather than dropped. They are the prompt that
    /// triggered this connection — opman rendered it optimistically, the agent has not
    /// received it, and so the replay will not contain it. Clearing it outright is what
    /// made a first message after a restart vanish from the transcript.
    ///
    /// Turn counters keep counting: the ids they generate must not collide with the ones
    /// already handed to the held messages and broadcast to the client.
    pub fn begin_replay(&mut self) {
        let split = self
            .messages
            .iter()
            .rposition(|m| m.info["role"] != "user")
            .map_or(0, |last| last + 1);
        self.held = self.messages.split_off(split);
        self.messages.clear();
        self.index.clear();
        self.tools.clear();
        self.live = None;
        self.open = None;
        self.replaying = true;
    }

    /// The replay is over: settle the last replayed message and put the held prompts back
    /// at the end, where they belong in time. They are not re-emitted — the client has
    /// rendered them since the moment they were typed.
    pub fn end_replay(&mut self, out: &mut Vec<Emit>) {
        self.replaying = false;
        self.finish_turn(out);
        for message in std::mem::take(&mut self.held) {
            if let Some(id) = message.info["id"].as_str() {
                self.index.insert(id.to_string(), self.messages.len());
            }
            self.messages.push(message);
        }
    }

    /// A user prompt. opman renders the prompt it sent optimistically, but the agent also
    /// replays user messages on `session/load`, so this is what rebuilds history.
    pub fn user_message(&mut self, prompt: &Prompt, out: &mut Vec<Emit>) -> String {
        // Replayed history is finished business, so a user turn ends the assistant turn
        // before it outright. Live, it only detaches: an agent that accepts a follow-up
        // mid-turn is still generating, and stamping it complete would settle tools that
        // are genuinely running.
        if self.replaying {
            self.finish_turn(out);
        }
        self.user_turn += 1;
        let mid = format!("msg_user_{}_{}", self.session_id, self.user_turn);
        let info = json!({
            "role": "user",
            "id": mid,
            "sessionID": self.session_id,
            "time": { "created": super::now_ms() },
        });
        // The attachments ride along as `file` parts, so the user's own bubble shows what
        // they sent rather than only the words that came with it.
        let parts = prompt.message_parts(&mid, &self.session_id);
        self.index.insert(mid.clone(), self.messages.len());
        self.messages.push(MsgOut {
            info: info.clone(),
            parts: parts.clone(),
        });
        // A user turn closes any assistant message that was still streaming.
        self.live = None;
        self.open = None;
        out.push(Emit::Message(info));
        for part in parts {
            out.push(Emit::Part(part));
        }
        mid
    }
}
