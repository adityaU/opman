//! Connection lifecycle: spawn an ACP server, negotiate, and drive turns.
//!
//! One pooled child per agent and working directory, but the semantics are the protocol's
//! rather than a pipe's:
//! - a follow-up mid-turn is another `session/prompt`, which agents that advertise
//!   steering deliver to the running model (true steering, not a queue);
//! - abort is `session/cancel`, so the agent unwinds and reports `stopReason: cancelled`
//!   instead of being killed and losing the turn;
//! - continuity after a restart is `session/load`, which replays history over
//!   `session/update` rather than re-reading a transcript file.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use tokio::sync::{Mutex, OnceCell};

use super::jsonrpc::Peer;
use super::AcpEngine;

/// Live children keyed by directory (or by session id when sharing is disabled), plus the
/// per-session binding that tells ACP traffic which session it belongs to.
#[derive(Default)]
pub struct ConnMap {
    /// The outer lock is held only while looking up or inserting a slot. The `OnceCell`
    /// makes concurrent first users await one spawn and handshake rather than racing them.
    pool: Mutex<HashMap<PoolKey, Slot>>,
    bound: Mutex<HashMap<String, Bound>>,
}

impl ConnMap {
    /// Detach a session from its child. The child is killed only when its last session leaves.
    pub async fn close(&self, session_id: &str) {
        let Some(bound) = self.bound.lock().await.remove(session_id) else {
            return;
        };
        let slot = self.pool.lock().await.get(&bound.key).cloned();
        let Some(slot) = slot else {
            return;
        };
        let Some(entry) = slot.get().cloned() else {
            return;
        };
        let mut pooled = entry.lock().await;
        if pooled.instance != bound.instance {
            return;
        }
        pooled.attached.remove(session_id);
        drop(pooled);
        if let Some(entry) = self.remove_entry(&bound.key, &slot).await {
            kill_entry(entry).await;
        }
    }

    /// The child is gone or wedged. Drop it and unbind every session on it, so each one
    /// reconnects (and `session/load`s) on its next prompt. Returns the other affected
    /// sessions so callers can tell their panes what happened.
    pub(super) async fn evict(&self, session_id: &str) -> Vec<String> {
        let Some(target) = self.bound.lock().await.get(session_id).cloned() else {
            return Vec::new();
        };
        let entry = {
            let mut pool = self.pool.lock().await;
            let Some(slot) = pool.get(&target.key).cloned() else {
                return Vec::new();
            };
            let Some(entry) = slot.get().cloned() else {
                return Vec::new();
            };
            if entry.lock().await.instance != target.instance {
                return Vec::new();
            }
            pool.remove(&target.key);
            entry
        };
        let attached: Vec<String> = entry.lock().await.attached.drain().collect();
        let mut bound = self.bound.lock().await;
        let mut affected = Vec::new();
        for id in &attached {
            if bound
                .get(id)
                .is_some_and(|current| current.instance == target.instance)
            {
                bound.remove(id);
                if id != session_id {
                    affected.push(id.clone());
                }
            }
        }
        drop(bound);
        kill_entry(entry).await;
        affected
    }

    /// Drop every connection, killing the children.
    ///
    /// Used when an agent is reconfigured or removed from `acp.json`: those processes were
    /// launched from the definition that no longer exists, so leaving them running would
    /// mean a session still talking to config the user just deleted.
    pub async fn close_all(&self) {
        let slots: Vec<_> = self
            .pool
            .lock()
            .await
            .drain()
            .map(|(_, slot)| slot)
            .collect();
        self.bound.lock().await.clear();
        for slot in slots {
            if let Some(entry) = slot.get() {
                kill_entry(entry.clone()).await;
            }
        }
    }

    /// The session's connection, establishing one on first use. Returns what a turn needs —
    /// never the pooled entry itself, so the child handle stays owned by the map and cannot be
    /// dropped (and so killed) by a caller holding it across an await.
    pub(super) async fn ensure(&self, engine: &Arc<AcpEngine>, session_id: &str) -> Result<Ready> {
        let session = engine
            .get_session(session_id)
            .context("unknown opman session")?;
        let dir = session.directory;
        if dir.is_empty() {
            bail!("session has no working directory");
        }
        let key = pool_key(engine, session_id, &dir);
        let slot = {
            let mut pool = self.pool.lock().await;
            pool.entry(key.clone())
                .or_insert_with(|| Arc::new(OnceCell::new()))
                .clone()
        };
        let engine_for_spawn = engine.clone();
        let dir_for_spawn = dir.clone();
        let entry = slot
            .get_or_try_init(|| async move {
                let pooled = spawn_and_negotiate(&engine_for_spawn, &dir_for_spawn).await?;
                Ok::<_, anyhow::Error>(Arc::new(Mutex::new(pooled)))
            })
            .await?
            .clone();

        let mut pooled = entry.lock().await;
        let current = self.bound.lock().await.get(session_id).cloned();
        if let Some(current) = current {
            if current.instance == pooled.instance {
                return Ok(ready(&pooled, &current.acp_session));
            }
        }

        let previous = { self.bound.lock().await.get(session_id).cloned() };
        if let Some(previous) = previous {
            self.detach_previous(session_id, &previous, &key).await;
        }

        let servers = engine.mcp_servers(&dir, session_id, pooled.negotiated.mcp_caps);
        let resume = session
            .acp_session
            .filter(|_| pooled.negotiated.loads)
            .filter(|id| !id.is_empty());
        let (acp_session, setup) = match resume {
            Some(prior) => {
                load_session(
                    engine,
                    &pooled.peer,
                    session_id,
                    &dir,
                    &prior,
                    &pooled.negotiated,
                    &servers,
                )
                .await?
            }
            None => open_session(&pooled.peer, &pooled.negotiated.init, &dir, &servers).await?,
        };
        engine.bind_acp_session(session_id, &acp_session);
        engine.merge_session_setup(session_id, &setup);
        super::conn_options::apply_defaults(engine, &pooled.peer, session_id, &acp_session, &setup)
            .await;
        pooled.attached.insert(session_id.to_string());
        self.bound.lock().await.insert(
            session_id.to_string(),
            Bound {
                key,
                instance: pooled.instance,
                acp_session: acp_session.clone(),
            },
        );
        Ok(ready(&pooled, &acp_session))
    }

    /// An already-established connection, or None. Used by abort, which must not start one.
    pub(super) async fn existing(&self, session_id: &str) -> Option<(Peer, String)> {
        let (key, instance, acp_session) = {
            let bound = self.bound.lock().await;
            let bound = bound.get(session_id)?;
            (bound.key.clone(), bound.instance, bound.acp_session.clone())
        };
        let slot = self.pool.lock().await.get(&key).cloned()?;
        let entry = slot.get().cloned()?;
        let pooled = entry.lock().await;
        if pooled.instance != instance {
            return None;
        }
        Some((pooled.peer.clone(), acp_session))
    }

    async fn remove_entry(&self, key: &str, expected: &Slot) -> Option<Arc<Mutex<Pooled>>> {
        let mut pool = self.pool.lock().await;
        if !pool
            .get(key)
            .is_some_and(|entry| Arc::ptr_eq(entry, expected))
        {
            return None;
        }
        let entry = expected.get()?.clone();
        if !entry.lock().await.attached.is_empty() {
            return None;
        }
        pool.remove(key);
        Some(entry)
    }

    async fn detach_previous(&self, session_id: &str, previous: &Bound, current_key: &str) {
        let removed = {
            let mut bound = self.bound.lock().await;
            if bound
                .get(session_id)
                .is_some_and(|current| current.instance == previous.instance)
            {
                bound.remove(session_id);
                true
            } else {
                false
            }
        };
        if !removed {
            return;
        }
        // The old child was already removed from this key's slot. There is no old
        // entry left to detach when a new child has taken the same key.
        if previous.key == current_key {
            return;
        }
        let Some(slot) = self.pool.lock().await.get(&previous.key).cloned() else {
            return;
        };
        let Some(entry) = slot.get().cloned() else {
            return;
        };
        {
            let mut pooled = entry.lock().await;
            if pooled.instance != previous.instance {
                return;
            }
            pooled.attached.remove(session_id);
        }
        if let Some(entry) = self.remove_entry(&previous.key, &slot).await {
            kill_entry(entry).await;
        }
    }
}

#[path = "conn_entry.rs"]
mod entry;
pub(super) use entry::Ready;
use entry::{kill_entry, pool_key, ready, Bound, PoolKey, Pooled, Slot, NEXT_INSTANCE_ID};

#[path = "conn_open.rs"]
mod open;
pub(super) use open::probe_capabilities;
use open::{load_session, open_session, spawn_and_negotiate};

#[cfg(all(test, unix))]
#[path = "conn_pool_tests.rs"]
mod conn_pool_tests;

#[cfg(all(test, unix))]
#[path = "conn_pool_more_tests.rs"]
mod conn_pool_more_tests;
