//! Prevent two limit events or browser clicks from forking the same source at once.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use anyhow::{bail, Result};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

type Guards = HashMap<(String, String), Weak<AsyncMutex<()>>>;

#[derive(Default)]
pub(in crate::runner) struct HandoffGuards(Mutex<Guards>);

impl HandoffGuards {
    pub(in crate::runner) fn acquire(
        &self,
        directory: &str,
        session_id: &str,
    ) -> Result<OwnedMutexGuard<()>> {
        let guard = {
            let mut guards = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
            guards.retain(|_, guard| guard.strong_count() > 0);
            let key = (directory.to_string(), session_id.to_string());
            match guards.get(&key).and_then(Weak::upgrade) {
                Some(guard) => guard,
                None => {
                    let guard = Arc::new(AsyncMutex::new(()));
                    guards.insert(key, Arc::downgrade(&guard));
                    guard
                }
            }
        };
        match guard.try_lock_owned() {
            Ok(held) => Ok(held),
            Err(_) => {
                bail!("This chat is already switching runners. Wait for the handoff to finish.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_only_block_the_same_source_and_prune_finished_sources() -> Result<()> {
        let guards = HandoffGuards::default();
        let first = guards.acquire("/project", "a")?;
        assert!(guards.acquire("/project", "a").is_err());
        let second = guards.acquire("/project", "b")?;
        drop(first);
        drop(second);
        let _next = guards.acquire("/project", "c")?;
        assert_eq!(
            guards
                .0
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .len(),
            1
        );
        Ok(())
    }
}
