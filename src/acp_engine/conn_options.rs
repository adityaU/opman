//! Pushing opman's session choices — mode, model, effort — onto a live ACP session.
//!
//! Split from [`super::conn`], which owns the connection's lifecycle. The distinction that
//! matters here is that a connection outlives many turns: an ACP child is spawned once and
//! then prompted repeatedly, so choices made after it started have to be pushed separately
//! rather than folded into `session/new`.

use std::sync::Arc;

use serde_json::{json, Value};
use tracing::debug;

use super::jsonrpc::Peer;
use super::AcpEngine;

/// Push opman's choices onto a session that has just been created or loaded. Best-effort: an
/// agent that has no such option simply errors, which is not a reason to fail the session.
pub(super) async fn apply_defaults(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    acp_session: &str,
    setup: &Value,
) {
    for (option, value) in engine.desired_options(session_id) {
        if !super::options::offers(setup, &option, &value) {
            continue;
        }
        push(engine, peer, session_id, acp_session, &option, &value).await;
    }
}

/// Re-push any choice the user has changed since the connection was opened.
///
/// Without this, [`apply_defaults`] fires once and everything chosen afterwards — a different
/// model, a different agent — sits in the session registry and never reaches the agent.
/// Comparing against the agent's own `currentValue` keeps this to a no-op, with no requests
/// at all, in the common case where nothing moved.
pub(super) async fn sync(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    acp_session: &str,
) {
    for (option, value) in engine.desired_options(session_id) {
        // Re-read per option: each push folds the agent's reply back in, so a later option
        // is compared against the state the earlier one left behind.
        let setup = engine.session_setup(session_id);
        if super::options::current(&setup, &option).as_deref() == Some(value.as_str()) {
            continue;
        }
        if !super::options::offers(&setup, &option, &value) {
            continue;
        }
        push(engine, peer, session_id, acp_session, &option, &value).await;
    }
}

/// Send one `session/set_config_option` and fold the agent's reply back into the stored
/// setup, so the next comparison sees what the agent actually settled on rather than what
/// opman asked for.
async fn push(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    acp_session: &str,
    option: &str,
    value: &str,
) {
    let params = json!({ "sessionId": acp_session, "configId": option, "value": value });
    match peer.request("session/set_config_option", params).await {
        Ok(reply) => engine.merge_config_list(session_id, &reply),
        Err(e) => debug!(session = %session_id, %option, "acp set_config_option failed: {e}"),
    }
}
