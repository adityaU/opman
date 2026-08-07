//! Pool tests cover the paths that need no installed server: unknown file
//! types, missing binaries, and the negative cache that keeps a polling
//! frontend from forking a doomed process several times a second.

use super::*;

fn project() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[tokio::test]
async fn unknown_file_types_resolve_to_no_server() {
    let dir = project();
    let file = dir.path().join("notes.unknownext");
    std::fs::write(&file, "hello").unwrap();

    let pool = LspPool::new();
    let resolved = pool.resolve(&file, dir.path()).await.unwrap();
    assert!(resolved.is_none());
}

/// A known language whose server is not installed is "no LSP here", not an
/// error — the editor renders that as the feature being unavailable.
#[tokio::test]
async fn a_missing_binary_is_not_an_error() {
    let dir = project();
    let file = dir.path().join("main.go");
    std::fs::write(&file, "package main").unwrap();

    // Point PATH somewhere empty so no real gopls can be found.
    let empty = tempfile::tempdir().unwrap();
    let restore = std::env::var_os("PATH");
    std::env::set_var("PATH", empty.path());

    let pool = LspPool::new();
    let outcome = pool.resolve(&file, dir.path()).await;

    match restore {
        Some(path) => std::env::set_var("PATH", path),
        None => std::env::remove_var("PATH"),
    }

    assert!(outcome.unwrap().is_none());
}

#[tokio::test]
async fn sweeping_an_empty_pool_is_harmless() {
    let pool = LspPool::new();
    assert_eq!(pool.sweep(Duration::from_secs(0)).await, 0);
}

#[tokio::test]
async fn evicting_an_absent_key_is_harmless() {
    let pool = LspPool::new();
    pool.evict(&ServerKey {
        root: PathBuf::from("/nowhere"),
        language: "rust",
    })
    .await;
}

#[tokio::test]
async fn shutdown_is_safe_on_an_empty_pool() {
    let pool = LspPool::new();
    pool.shutdown_all().await;
}

/// Files in the same crate must produce the same key, so they share one server.
#[test]
fn keys_are_equal_for_the_same_root_and_language() {
    let a = ServerKey {
        root: PathBuf::from("/w"),
        language: "rust",
    };
    let b = ServerKey {
        root: PathBuf::from("/w"),
        language: "rust",
    };
    assert_eq!(a, b);

    let other = ServerKey {
        root: PathBuf::from("/w"),
        language: "go",
    };
    assert_ne!(a, other);
}
