//! Wave-2 tests driving the local-managed setup helpers (`create_tunnel`,
//! `route_dns`, `verify_tunnel`) with a FAKE `cloudflared` on PATH.

use super::*;
use crate::web::tunnel::types::test_env_support::{env_lock, write_fake_bin, EnvRestore};

fn setup_fake(script: &str) -> (tempfile::TempDir, EnvRestore) {
    let dir = tempfile::tempdir().unwrap();
    write_fake_bin(dir.path(), "cloudflared", script);
    let mut env = EnvRestore::new();
    env.prepend_path(dir.path());
    (dir, env)
}

// ── create_tunnel ───────────────────────────────────────────────────

#[tokio::test]
async fn create_tunnel_writes_cred_and_returns_uuid() {
    let _g = env_lock();
    // Fake finds --cred-file <path>, writes a tunnel.json there, exits 0.
    let (_d, _env) = setup_fake(
        "prev=\"\"\nfor a in \"$@\"; do\n  if [ \"$prev\" = \"--cred-file\" ]; then printf '{\"TunnelID\":\"gen-uuid-1\"}' > \"$a\"; fi\n  prev=\"$a\"\ndone\nexit 0",
    );
    let work = tempfile::tempdir().unwrap();
    let cert = work.path().join("cert.pem");
    let cred = work.path().join("tunnel.json");
    let uuid = create_tunnel(&cert, &cred, "opman").await.unwrap();
    assert_eq!(uuid, "gen-uuid-1");
    assert!(cred.exists());
}

#[tokio::test]
async fn create_tunnel_nonzero_exit_is_error() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("exit 1");
    let work = tempfile::tempdir().unwrap();
    let err = create_tunnel(
        &work.path().join("cert.pem"),
        &work.path().join("t.json"),
        "opman",
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("Failed to create tunnel"), "got: {err}");
}

// ── route_dns ───────────────────────────────────────────────────────

#[tokio::test]
async fn route_dns_success() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("exit 0");
    let work = tempfile::tempdir().unwrap();
    route_dns(&work.path().join("cert.pem"), "uuid-x", "host.example.com")
        .await
        .unwrap();
}

#[tokio::test]
async fn route_dns_nonzero_exit_is_error() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("exit 3");
    let work = tempfile::tempdir().unwrap();
    let err = route_dns(&work.path().join("cert.pem"), "uuid-x", "host.example.com")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Failed to create DNS entry"), "got: {err}");
}

// ── verify_tunnel ───────────────────────────────────────────────────

#[tokio::test]
async fn verify_tunnel_matching_name_ok() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("printf '[{\"name\":\"opman\"}]'\nexit 0");
    let work = tempfile::tempdir().unwrap();
    verify_tunnel(&work.path().join("cert.pem"), "uuid-1", "opman")
        .await
        .unwrap();
}

#[tokio::test]
async fn verify_tunnel_name_mismatch_errors() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("printf '[{\"name\":\"other\"}]'\nexit 0");
    let work = tempfile::tempdir().unwrap();
    let err = verify_tunnel(&work.path().join("cert.pem"), "uuid-1", "opman")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("name mismatch"), "got: {err}");
}

#[tokio::test]
async fn verify_tunnel_empty_list_errors() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("printf '[]'\nexit 0");
    let work = tempfile::tempdir().unwrap();
    let err = verify_tunnel(&work.path().join("cert.pem"), "uuid-1", "opman")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("not found"), "got: {err}");
}

#[tokio::test]
async fn verify_tunnel_nonzero_exit_errors() {
    let _g = env_lock();
    let (_d, _env) = setup_fake("printf 'boom' >&2\nexit 1");
    let work = tempfile::tempdir().unwrap();
    let err = verify_tunnel(&work.path().join("cert.pem"), "uuid-1", "opman")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("tunnel list failed"), "got: {err}");
}

#[tokio::test]
async fn verify_tunnel_missing_name_field_treated_as_mismatch() {
    let _g = env_lock();
    // name field absent -> unwrap_or("") -> mismatch with expected.
    let (_d, _env) = setup_fake("printf '[{\"id\":\"x\"}]'\nexit 0");
    let work = tempfile::tempdir().unwrap();
    let err = verify_tunnel(&work.path().join("cert.pem"), "uuid-1", "opman")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("name mismatch"), "got: {err}");
}
