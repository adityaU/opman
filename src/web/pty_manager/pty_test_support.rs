//! Test-only helpers for driving the PTY spawn code with FAKE programs on
//! `PATH`/`$SHELL`. Mirrors the tunnel-side helper but lives in the
//! `pty_manager` subtree so it is reachable from these test modules. Env vars
//! are process-global, so callers MUST hold [`env_lock`] for the whole test.

use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

/// Acquire the crate-wide env serialization lock (poison-tolerant). Shared with
/// the tunnel/claude-cli env tests so all env mutation is globally serialized.
pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    crate::claude_engine::claude_cli::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// RAII guard that records and restores env vars it changed.
pub(crate) struct EnvRestore {
    saved: Vec<(String, Option<String>)>,
}

impl EnvRestore {
    pub(crate) fn new() -> Self {
        EnvRestore { saved: Vec::new() }
    }

    pub(crate) fn set(&mut self, key: &str, val: &str) {
        self.saved.push((key.to_string(), std::env::var(key).ok()));
        std::env::set_var(key, val);
    }

    pub(crate) fn remove(&mut self, key: &str) {
        self.saved.push((key.to_string(), std::env::var(key).ok()));
        std::env::remove_var(key);
    }

    pub(crate) fn prepend_path(&mut self, dir: &Path) {
        let old = std::env::var("PATH").unwrap_or_default();
        let new = format!("{}:{}", dir.display(), old);
        self.set("PATH", &new);
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (k, v) in self.saved.iter().rev() {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}

/// Write an executable `#!/bin/sh` script named `name` into `dir`; return path.
pub(crate) fn write_fake_bin(dir: &Path, name: &str, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
    let mut perms = std::fs::metadata(&p).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).unwrap();
    p
}
