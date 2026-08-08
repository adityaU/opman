//! Questions raised through the `ask` MCP server and waiting on a human.
//!
//! The engine-side equivalent is [`crate::acp_engine::pending`]; this one exists because
//! the asker is not an engine at all. The `ask` server runs as a child of the *runner*,
//! reaches opman over the loopback API, and is answered by whichever browser is looking —
//! so the waiter has to live where both the HTTP request and the reply route can see it,
//! which is the web server.
//!
//! Keyed by request id and tagged with the session, so aborting a session can answer every
//! card that session left on screen rather than leaving them to age out.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::oneshot;

/// What the user chose, per question, in the order asked. Empty means dismissed.
pub type Answers = Vec<Vec<String>>;

struct Waiter {
    session: String,
    reply: oneshot::Sender<Answers>,
}

/// The open questions for this opman process.
#[derive(Default)]
pub struct AskPending(Mutex<HashMap<String, Waiter>>);

impl AskPending {
    /// Park a request. The returned receiver resolves when the user answers, or errors
    /// when the waiter is dropped — which is how a dismiss reaches the asker.
    pub fn register(&self, id: &str, session: &str) -> oneshot::Receiver<Answers> {
        let (reply, rx) = oneshot::channel();
        let waiter = Waiter {
            session: session.to_string(),
            reply,
        };
        if let Ok(mut open) = self.0.lock() {
            open.insert(id.to_string(), waiter);
        }
        rx
    }

    /// Answer one request. `Err` hands the answers back rather than dropping them: nothing
    /// here was waiting on this id, so the reply route still has to fan it out to the
    /// engines, and it needs the answers to do that.
    pub fn resolve(&self, id: &str, answers: Answers) -> Result<(), Answers> {
        let Some(waiter) = self.take(id) else {
            return Err(answers);
        };
        waiter.reply.send(answers)
    }

    /// Drop a request without answering it. The asker sees a closed channel and is told
    /// the user dismissed the question.
    pub fn dismiss(&self, id: &str) -> bool {
        self.take(id).is_some()
    }

    /// Dismiss everything a session still has open, returning the ids so the caller can
    /// tell the clients to drop the cards.
    pub fn clear_session(&self, session: &str) -> Vec<String> {
        let Ok(mut open) = self.0.lock() else {
            return Vec::new();
        };
        let cleared: Vec<String> = open
            .iter()
            .filter(|(_, waiter)| waiter.session == session)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &cleared {
            open.remove(id);
        }
        cleared
    }

    fn take(&self, id: &str) -> Option<Waiter> {
        self.0.lock().ok().and_then(|mut open| open.remove(id))
    }
}

#[cfg(test)]
#[path = "ask_pending_tests.rs"]
mod ask_pending_tests;
