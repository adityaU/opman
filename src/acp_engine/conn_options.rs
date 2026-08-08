//! Pushing opman's session choices — mode, model, effort — onto a live ACP session.
//!
//! Split from [`super::conn`], which owns the connection's lifecycle. The distinction that
//! matters here is that a connection outlives many turns: an ACP child is spawned once and
//! then prompted repeatedly, so choices made after it started have to be pushed separately
//! rather than folded into `session/new`.

use std::sync::Arc;

use serde_json::Value;
use tracing::debug;

use super::jsonrpc::Peer;
use super::options::Channel;
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
        let Some(channel) = super::options::channel_for(setup, &option, &value) else {
            continue;
        };
        push(
            engine,
            peer,
            session_id,
            acp_session,
            channel,
            &option,
            &value,
        )
        .await;
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
        if super::options::selected(&setup, &option).as_deref() == Some(value.as_str()) {
            continue;
        }
        let Some(channel) = super::options::channel_for(&setup, &option, &value) else {
            continue;
        };
        push(
            engine,
            peer,
            session_id,
            acp_session,
            channel,
            &option,
            &value,
        )
        .await;
    }
}

/// Send one choice on the channel the agent publishes it on, and fold the outcome back into
/// the stored setup so the next comparison sees what the agent settled on rather than what
/// opman asked for.
///
/// A "method not found" is treated as a wrong guess rather than a failure: publishing the
/// spec's `modes` without serving `session/set_mode` happens, and where the same value is also
/// a config option that is where the agent really wants it.
async fn push(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    acp_session: &str,
    channel: Channel,
    option: &str,
    value: &str,
) {
    let failure = match send(
        engine,
        peer,
        session_id,
        acp_session,
        channel,
        option,
        value,
    )
    .await
    {
        Ok(()) => return,
        Err(failure) => failure,
    };
    let setup = engine.session_setup(session_id);
    let fallback = (channel != Channel::Config && super::jsonrpc::unimplemented(&failure))
        .then(|| super::options::config_channel(&setup, option, value))
        .flatten();
    let Some(fallback) = fallback else {
        debug!(session = %session_id, %option, method = channel.method(), "acp set failed: {failure}");
        return;
    };
    if let Err(failure) = send(
        engine,
        peer,
        session_id,
        acp_session,
        fallback,
        option,
        value,
    )
    .await
    {
        debug!(session = %session_id, %option, method = fallback.method(), "acp set failed: {failure}");
    }
}

async fn send(
    engine: &Arc<AcpEngine>,
    peer: &Peer,
    session_id: &str,
    acp_session: &str,
    channel: Channel,
    option: &str,
    value: &str,
) -> anyhow::Result<()> {
    let reply = peer
        .request(channel.method(), channel.params(acp_session, option, value))
        .await?;
    engine.note_selected(session_id, channel, option, value, &reply);
    Ok(())
}

#[cfg(test)]
#[path = "conn_options_tests.rs"]
mod conn_options_tests;
