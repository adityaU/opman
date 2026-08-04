//! Wave-2 tests driving the `spawn_tunnel` entry point with a FAKE `cloudflared`
//! on PATH (Technique 2). Covers the success dispatch (Quick), the spawn-failure
//! branch (binary absent), and `TunnelHandle::drop` killing a real child.

use super::test_env_support::{env_lock, write_fake_bin, EnvRestore};
use super::*;

#[tokio::test]
async fn spawn_tunnel_quick_success_returns_live_handle() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    // Fake cloudflared prints the tunnel URL to stderr, then stays alive so the
    // handle has a real child to kill on drop.
    write_fake_bin(
        dir.path(),
        "cloudflared",
        "printf 'INF |  https://gen-test.trycloudflare.com  |\\n' >&2\nsleep 5",
    );
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let handle = spawn_tunnel(TunnelMode::Quick, 8080, &TunnelOptions::default()).await;
    // A child was spawned successfully.
    assert!(handle.child.is_some(), "quick tunnel should have a child");
    // Drop kills the real cloudflared child (exercises Drop's start_kill arm).
    drop(handle);
}

#[tokio::test]
async fn spawn_tunnel_named_success_returns_live_handle() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(
        dir.path(),
        "cloudflared",
        "printf 'Registered tunnel connection idx=0\\n' >&2\nsleep 5",
    );
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let handle = spawn_tunnel(
        TunnelMode::Named {
            token: "tok".into(),
        },
        9000,
        &TunnelOptions::default(),
    )
    .await;
    assert!(handle.child.is_some());
    // Give the background stderr reader a moment to consume the "Registered" line.
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    drop(handle);
}

#[tokio::test]
async fn spawn_tunnel_spawn_failure_yields_childless_handle() {
    let _g = env_lock();
    // PATH points ONLY at an empty dir — cloudflared cannot be found, so the
    // spawn fails and spawn_tunnel returns a handle with no child.
    let dir = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &dir.path().display().to_string());

    let handle = spawn_tunnel(TunnelMode::Quick, 8080, &TunnelOptions::default()).await;
    assert!(handle.child.is_none(), "no binary -> no child");
    assert!(handle._config_file.is_none());
    drop(handle); // Drop with child None is a no-op.
}
