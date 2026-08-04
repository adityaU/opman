//! Wave-2 tests driving `spawn_local_managed` and `ensure_certificate` with a
//! FAKE `cloudflared` on PATH plus a temp `HOME`/`XDG_CONFIG_HOME` (so the real
//! `~/.config/opman/tunnel` is never touched).

use super::*;
use crate::web::tunnel::types::test_env_support::{env_lock, write_fake_bin, EnvRestore};

/// A `cloudflared` that dispatches on its subcommand. `list_name` is what
/// `tunnel list` reports (used to force verify success/mismatch).
fn dispatch_script(list_name: &str) -> String {
    format!(
        "case \" $* \" in\n\
         *\" login \"*) mkdir -p \"$HOME/.cloudflared\"; printf cert > \"$HOME/.cloudflared/cert.pem\"; exit 0;;\n\
         *\" create \"*) prev=\"\"; for a in \"$@\"; do if [ \"$prev\" = \"--cred-file\" ]; then printf '{{\"TunnelID\":\"gen-uuid-1\"}}' > \"$a\"; fi; prev=\"$a\"; done; exit 0;;\n\
         *\" route \"*) exit 0;;\n\
         *\" list \"*) printf '[{{\"name\":\"{list_name}\"}}]'; exit 0;;\n\
         *\" run \"*) printf 'Registered tunnel connection\\n' >&2; printf 'transient error retrying\\n' >&2; sleep 5; exit 0;;\n\
         *) exit 0;;\n\
         esac"
    )
}

struct Harness {
    _base: tempfile::TempDir,
    data_dir: std::path::PathBuf,
    _env: EnvRestore,
}

fn harness(list_name: &str) -> Harness {
    let base = tempfile::tempdir().unwrap();
    let cfg = base.path().join("config");
    let home = base.path().join("home");
    let bin = base.path().join("bin");
    for d in [&cfg, &home, &bin] {
        std::fs::create_dir_all(d).unwrap();
    }
    write_fake_bin(&bin, "cloudflared", &dispatch_script(list_name));
    let mut env = EnvRestore::new();
    env.set("HOME", &home.display().to_string());
    env.set("XDG_CONFIG_HOME", &cfg.display().to_string());
    env.prepend_path(&bin);
    let data_dir = cfg.join("opman").join("tunnel");
    Harness {
        _base: base,
        data_dir,
        _env: env,
    }
}

#[tokio::test]
async fn spawn_local_managed_fresh_full_flow() {
    let _g = env_lock();
    let h = harness("opman");
    // No cert, no tunnel.json -> login + create + route + generate + run.
    let (mut child, cfg) =
        spawn_local_managed("host.example.com", "opman", 8080, &TunnelOptions::default())
            .await
            .unwrap();
    let cfg = cfg.expect("local-managed returns a config path");
    assert!(cfg.exists(), "config.json should be generated");
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_local_managed_existing_cert_and_valid_tunnel() {
    let _g = env_lock();
    let h = harness("opman");
    // Pre-seed cert + tunnel.json so the "existing" + verify-ok branches run.
    std::fs::create_dir_all(&h.data_dir).unwrap();
    std::fs::write(h.data_dir.join("cert.pem"), b"cert").unwrap();
    std::fs::write(
        h.data_dir.join("tunnel.json"),
        br#"{"TunnelID":"existing-uuid"}"#,
    )
    .unwrap();

    let opts = TunnelOptions {
        protocol: Some("http2".into()),
        region: Some("us".into()),
        edge_ips: vec![],
    };
    let (mut child, cfg) = spawn_local_managed("host.example.com", "opman", 9090, &opts)
        .await
        .unwrap();
    assert!(cfg.unwrap().exists());
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_local_managed_existing_tunnel_fails_verify_recreates() {
    let _g = env_lock();
    // list reports a different name -> verify_tunnel fails -> recreate branch.
    let h = harness("some-other-name");
    std::fs::create_dir_all(&h.data_dir).unwrap();
    std::fs::write(h.data_dir.join("cert.pem"), b"cert").unwrap();
    std::fs::write(
        h.data_dir.join("tunnel.json"),
        br#"{"TunnelID":"stale-uuid"}"#,
    )
    .unwrap();

    let (mut child, cfg) =
        spawn_local_managed("host.example.com", "opman", 7070, &TunnelOptions::default())
            .await
            .unwrap();
    assert!(cfg.unwrap().exists());
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let _ = child.start_kill();
}

#[tokio::test]
async fn spawn_local_managed_spawn_failure_when_binary_absent() {
    let _g = env_lock();
    // Empty PATH (no cloudflared) — ensure_certificate's spawn fails -> Err.
    let base = tempfile::tempdir().unwrap();
    let cfg = base.path().join("config");
    let home = base.path().join("home");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let mut env = EnvRestore::new();
    env.set("HOME", &home.display().to_string());
    env.set("XDG_CONFIG_HOME", &cfg.display().to_string());
    env.set("PATH", &base.path().join("empty-bin").display().to_string());

    let res = spawn_local_managed("h", "opman", 8080, &TunnelOptions::default()).await;
    assert!(res.is_err(), "no cloudflared -> error");
}

// ── ensure_certificate direct ───────────────────────────────────────

#[tokio::test]
async fn ensure_certificate_success_copies_cert() {
    let _g = env_lock();
    let h = harness("opman");
    std::fs::create_dir_all(&h.data_dir).unwrap();
    let cert = h.data_dir.join("cert.pem");
    ensure_certificate(&cert).await.unwrap();
    assert!(cert.exists(), "cert copied from $HOME/.cloudflared");
}

#[tokio::test]
async fn ensure_certificate_login_nonzero_exit_errors() {
    let _g = env_lock();
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let bin = base.path().join("bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    write_fake_bin(&bin, "cloudflared", "exit 1");
    let mut env = EnvRestore::new();
    env.set("HOME", &home.display().to_string());
    env.prepend_path(&bin);

    let cert = base.path().join("cert.pem");
    let err = ensure_certificate(&cert).await.unwrap_err().to_string();
    assert!(err.contains("login failed"), "got: {err}");
}

#[tokio::test]
async fn ensure_certificate_login_ok_but_cert_missing_errors() {
    let _g = env_lock();
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let bin = base.path().join("bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    // Exits 0 but never produces ~/.cloudflared/cert.pem.
    write_fake_bin(&bin, "cloudflared", "exit 0");
    let mut env = EnvRestore::new();
    env.set("HOME", &home.display().to_string());
    env.prepend_path(&bin);

    let cert = base.path().join("cert.pem");
    let err = ensure_certificate(&cert).await.unwrap_err().to_string();
    assert!(err.contains("was not found"), "got: {err}");
}
