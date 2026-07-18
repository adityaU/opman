//! Wave-2 tests driving `spawn_named` end-to-end with a FAKE `cloudflared`.

use super::*;
use crate::web::tunnel::types::test_env_support::{env_lock, write_fake_bin, EnvRestore};

#[tokio::test]
async fn spawn_named_success_and_reader_matches_registered_line() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(
        dir.path(),
        "cloudflared",
        // One noise line, then a "Registered tunnel connection" line the reader
        // task recognizes, then stay alive.
        "printf 'INF booting\\n' >&2\nprintf 'Registered tunnel connection idx=1\\n' >&2\nsleep 5",
    );
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let (mut child, cfg) = spawn_named("my-token", 8443, &TunnelOptions::default())
        .await
        .unwrap();
    assert!(cfg.is_none());
    // Let the spawned stderr reader consume the recognized line.
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_named_success_with_connection_registered_variant() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(
        dir.path(),
        "cloudflared",
        "printf 'Connection registered connIndex=0\\n' >&2\nsleep 5",
    );
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());

    let opts = TunnelOptions {
        protocol: Some("http2".into()),
        region: Some("us".into()),
        edge_ips: vec!["1.2.3.4:7844".into()],
    };
    let (mut child, _cfg) = spawn_named("tok2", 8444, &opts).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_named_spawn_failure_is_error() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let mut env = EnvRestore::new();
    env.set("PATH", &dir.path().display().to_string());
    assert!(spawn_named("t", 8000, &TunnelOptions::default()).await.is_err());
}
