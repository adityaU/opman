//! The token store: expiry states, file permissions, and refresh serialisation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;
use crate::mcp_oauth::ServerName;

fn name() -> ServerName {
    ServerName::parse("linear").expect("valid")
}

fn record(expires_at: Option<u64>, refresh: Option<&str>) -> TokenRecord {
    TokenRecord {
        version: 1,
        resource: "https://x/mcp".into(),
        issuer: "https://as".into(),
        client_id: "cid".into(),
        client_secret: None,
        access_token: Secret::new("at"),
        refresh_token: refresh.map(Secret::new),
        token_endpoint: "https://as/token".into(),
        expires_at,
        granted_scopes: Vec::new(),
        requested_scopes: Vec::new(),
    }
}

fn temp_store(tag: &str) -> (TokenStore, PathBuf) {
    let dir = std::env::temp_dir().join(format!("opman-oauth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create");
    (TokenStore::with_dir(dir.clone()), dir)
}

// ── expiry ───────────────────────────────────────────────────────────────────────────

#[test]
fn a_token_inside_the_skew_window_counts_as_expired() {
    let now = 1_000;
    // A request in flight must not outlive its own credential.
    assert!(matches!(
        record(Some(now + SKEW_SECS - 1), Some("rt")).credential(now),
        Credential::Refreshable(_)
    ));
    assert!(matches!(
        record(Some(now + SKEW_SECS + 1), Some("rt")).credential(now),
        Credential::Fresh(_)
    ));
}

#[test]
fn no_expiry_means_fresh() {
    assert!(matches!(
        record(None, None).credential(1_000),
        Credential::Fresh(_)
    ));
}

#[test]
fn expired_without_a_refresh_token_is_unusable() {
    assert!(matches!(
        record(Some(1), None).credential(1_000),
        Credential::Unusable
    ));
}

// ── persistence ──────────────────────────────────────────────────────────────────────

#[test]
fn a_saved_record_round_trips() {
    let (store, dir) = temp_store("roundtrip");
    store.save(&name(), &record(Some(9_999), Some("rt"))).expect("save");
    let loaded = store.load(&name()).expect("load");
    assert_eq!(loaded.access_token.expose(), "at");
    assert_eq!(loaded.refresh_token.expect("rt").expose(), "rt");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn a_token_file_is_created_private() {
    use std::os::unix::fs::PermissionsExt;
    let (store, dir) = temp_store("perms");
    store.save(&name(), &record(None, None)).expect("save");
    let mode = std::fs::metadata(dir.join("linear.json"))
        .expect("stat")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
    let _ = std::fs::remove_dir_all(dir);
}

/// A file left readable by an earlier version is repaired rather than trusted silently —
/// the pattern this store is modelled on wrote 0644.
#[cfg(unix)]
#[test]
fn a_world_readable_file_is_tightened_on_load() {
    use std::os::unix::fs::PermissionsExt;
    let (store, dir) = temp_store("repair");
    store.save(&name(), &record(None, None)).expect("save");
    let path = dir.join("linear.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    assert!(store.load(&name()).is_some());
    let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleting_a_missing_record_is_not_an_error() {
    let (store, dir) = temp_store("delete");
    assert!(store.delete(&name()).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_secret_does_not_print_itself() {
    // This codebase logs heavily; a derived Debug would leak a token into the log.
    let printed = format!("{:?}", Secret::new("super-secret"));
    assert!(!printed.contains("super-secret"), "{printed}");
}

// ── refresh serialisation ────────────────────────────────────────────────────────────

/// Two proxies refreshing at once would present the same single-use refresh token, and a
/// strict authorization server revokes the whole grant for that. The lock plus a
/// double-check means only one exchange happens.
#[tokio::test]
async fn concurrent_refreshes_run_the_exchange_exactly_once() {
    let (store, dir) = temp_store("refresh-once");
    let store = Arc::new(store);
    store.save(&name(), &record(Some(1), Some("rt"))).expect("save");

    let calls = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let store = Arc::clone(&store);
        let calls = Arc::clone(&calls);
        handles.push(tokio::spawn(async move {
            store
                .refresh_once(&name(), 1_000, |mut old| {
                    let calls = Arc::clone(&calls);
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        old.access_token = Secret::new("refreshed");
                        old.expires_at = Some(9_999);
                        Ok(old)
                    }
                })
                .await
                .map(|r| r.access_token.expose().to_string())
        }));
    }
    for handle in handles {
        let token = handle.await.expect("join").expect("refresh");
        // Every loser observes the winner's token rather than burning its own.
        assert_eq!(token, "refreshed");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn refresh_once_skips_the_exchange_when_the_record_is_already_fresh() {
    let (store, dir) = temp_store("already-fresh");
    store.save(&name(), &record(Some(9_999), Some("rt"))).expect("save");
    let calls = AtomicUsize::new(0);
    let out = store
        .refresh_once(&name(), 1_000, |old| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move { Ok(old) }
        })
        .await
        .expect("ok");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(out.access_token.expose(), "at");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn server_names_are_validated() {
    assert!(ServerName::parse("linear").is_ok());
    for bad in ["", "..", "a/b", &"x".repeat(65)] {
        assert!(ServerName::parse(bad).is_err(), "{bad:?} must be rejected");
    }
}
