//! Wave-3 tests driving `ensure_certificate`'s auth-URL extraction branches:
//! a fake `cloudflared tunnel login` that prints a Cloudflare auth URL to BOTH
//! stdout (login.cloudflareaccess.org variant) and stderr (dash.cloudflare.com
//! variant), so both the stdout-drain and stderr-drain URL blocks run. A fake
//! `xdg-open` on PATH makes `open::that` deterministic (no real browser).

use super::*;
use crate::web::tunnel::types::test_env_support::{env_lock, write_fake_bin, EnvRestore};

/// A `cloudflared login` fake that emits auth URLs on stdout+stderr and writes
/// the cert to `$HOME/.cloudflared/cert.pem`. `xdg_exit` sets the exit code of
/// the fake `xdg-open` used by `open::that` (0 → Ok branch, non-zero → Err branch).
fn login_env(xdg_exit: i32) -> (tempfile::TempDir, EnvRestore, std::path::PathBuf) {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let bin = base.path().join("bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();

    write_fake_bin(
        &bin,
        "cloudflared",
        "printf 'https://login.cloudflareaccess.org/abc123\\n'\n\
         printf 'Please open the following URL:\\n' >&2\n\
         printf 'https://dash.cloudflare.com/argotunnel?aud=z\\n' >&2\n\
         mkdir -p \"$HOME/.cloudflared\"\n\
         printf cert > \"$HOME/.cloudflared/cert.pem\"\n\
         exit 0",
    );
    // Fake browser opener so open::that resolves against PATH deterministically.
    write_fake_bin(&bin, "xdg-open", &format!("exit {xdg_exit}"));

    let mut env = EnvRestore::new();
    env.set("HOME", &home.display().to_string());
    // PATH: our fakes first, then standard dirs so `mkdir`/`printf` resolve.
    env.set("PATH", &format!("{}:/usr/bin:/bin", bin.display()));
    let cert = base.path().join("data").join("cert.pem");
    (base, env, cert)
}

#[tokio::test]
async fn ensure_certificate_extracts_auth_url_and_opens_browser_ok() {
    let _g = env_lock();
    let (_base, _env, cert) = login_env(0);
    ensure_certificate(&cert).await.unwrap();
    assert!(cert.exists(), "cert copied after login + URL extraction");
}

#[tokio::test]
async fn ensure_certificate_extracts_auth_url_open_failure_branch() {
    let _g = env_lock();
    // xdg-open exits non-zero → open::that returns Err → the "could not open"
    // fallback branches run (both stdout and stderr URL blocks).
    let (_base, _env, cert) = login_env(7);
    ensure_certificate(&cert).await.unwrap();
    assert!(cert.exists());
}
