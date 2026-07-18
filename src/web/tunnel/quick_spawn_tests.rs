//! Wave-2 tests driving `spawn_quick` end-to-end with a FAKE `cloudflared`.

use super::*;
use crate::web::tunnel::types::test_env_support::{env_lock, write_fake_bin, EnvRestore};

#[tokio::test]
async fn spawn_quick_detects_tunnel_url() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(
        dir.path(),
        "cloudflared",
        // Print a non-URL line first (drives the debug branch), then the URL.
        "printf 'INF starting\\n' >&2\nprintf '  https://abc.trycloudflare.com \\n' >&2\nsleep 5",
    );
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let (mut child, cfg) = spawn_quick(7100, &TunnelOptions::default()).await.unwrap();
    assert!(cfg.is_none());
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_quick_handles_exit_before_url() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    // Prints a line with no tunnel URL, then exits — drives the Ok(None) EOF arm.
    write_fake_bin(dir.path(), "cloudflared", "printf 'no url here\\n' >&2");
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let (mut child, cfg) = spawn_quick(7101, &TunnelOptions::default()).await.unwrap();
    assert!(cfg.is_none());
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_quick_spawn_failure_is_error() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &dir.path().display().to_string());
    // cloudflared not on PATH -> spawn() errors.
    assert!(spawn_quick(7102, &TunnelOptions::default()).await.is_err());
}
