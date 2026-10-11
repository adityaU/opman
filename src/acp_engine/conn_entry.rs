//! The pool's entries: one live ACP child, the per-session binding to it, and what a turn
//! is handed once a connection is ready.

use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::{Mutex, OnceCell};

use super::super::attach::PromptCaps;
use super::super::jsonrpc::Peer;
use super::super::AcpEngine;

pub(super) type PoolKey = String;
/// One pool entry: concurrent first users await the same spawn through the `OnceCell`.
pub(super) type Slot = Arc<OnceCell<Arc<Mutex<Pooled>>>>;

pub(super) static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

/// A live ACP server shared by one or more opman sessions.
pub(super) struct Pooled {
    pub(super) peer: Peer,
    pub(super) child: Child,
    /// Monotonic identity of this live child, distinct from its pool key.
    pub(super) instance: u64,
    /// Capabilities settled by the process handshake, reused by every session it serves.
    pub(super) negotiated: super::super::handshake::Negotiated,
    /// Opman sessions currently bound to this child.
    pub(super) attached: HashSet<String>,
}

#[derive(Clone)]
pub(super) struct Bound {
    pub(super) key: PoolKey,
    /// The live child this binding belongs to.
    pub(super) instance: u64,
    /// The agent's own session id, used in every `session/*` call.
    pub(super) acp_session: String,
}

/// What a turn needs from an established connection.
pub(in super::super) struct Ready {
    pub peer: Peer,
    pub acp_session: String,
    pub steering: bool,
    pub prompt_caps: PromptCaps,
}

pub(super) fn pool_key(engine: &Arc<AcpEngine>, session_id: &str, dir: &str) -> PoolKey {
    if engine.agent.shared_process {
        dir.to_string()
    } else {
        session_id.to_string()
    }
}

pub(super) fn ready(pooled: &Pooled, acp_session: &str) -> Ready {
    Ready {
        peer: pooled.peer.clone(),
        acp_session: acp_session.to_string(),
        steering: pooled.negotiated.steering,
        prompt_caps: pooled.negotiated.prompt_caps,
    }
}

pub(super) async fn kill_entry(entry: Arc<Mutex<Pooled>>) {
    let mut pooled = entry.lock().await;
    let _ = pooled.child.start_kill();
    let _ = pooled.child.wait().await;
}
