use super::*;

fn password_of(credentials: &Credentials) -> Option<(&str, &str)> {
    match credentials {
        Credentials::Password { username, password } => Some((username, password.expose())),
        Credentials::Open => None,
    }
}

fn locked() -> Credentials {
    Credentials::Password {
        username: "me".into(),
        password: Secret::new("old"),
    }
}

#[test]
fn credentials_absent_password_keeps_stored() {
    let merged = credentials(Some("me".into()), None, Some(&locked()));
    assert_eq!(password_of(&merged), Some(("me", "old")));
    let renamed = credentials(Some("you".into()), None, Some(&locked()));
    assert_eq!(password_of(&renamed), Some(("you", "old")));
}

#[test]
fn credentials_new_password_replaces() {
    let merged = credentials(None, Some(Secret::new("new")), Some(&locked()));
    assert_eq!(password_of(&merged), Some(("me", "new")));
}

#[test]
fn credentials_empty_username_means_open() {
    assert!(password_of(&credentials(Some("  ".into()), None, Some(&locked()))).is_none());
    assert!(password_of(&credentials(None, Some(Secret::new("x")), None)).is_none());
    assert!(password_of(&credentials(None, None, Some(&Credentials::Open))).is_none());
}

#[test]
fn clean_name_trims_and_rejects_blank() {
    assert_eq!(clean_name("  Dev ").unwrap(), "Dev");
    assert!(matches!(clean_name("   "), Err(RemoteError::InvalidName)));
}

#[tokio::test]
async fn bad_input_is_rejected_before_any_network() {
    let registry = Registry::new(ServerStore::memory());
    let blank = NewServer {
        name: " ".into(),
        url: "http://h".into(),
        username: None,
        password: None,
    };
    assert!(matches!(registry.add(blank).await, Err(RemoteError::InvalidName)));
    let bad_url = NewServer {
        name: "x".into(),
        url: "nope".into(),
        username: None,
        password: None,
    };
    assert!(matches!(registry.add(bad_url).await, Err(RemoteError::InvalidUrl(_))));
    let unknown = ServerId::parse("ghost").unwrap();
    assert!(matches!(
        registry.remove(&unknown).await,
        Err(RemoteError::UnknownServer)
    ));
    assert!(matches!(
        registry.update(&unknown, ServerPatch::default()).await,
        Err(RemoteError::UnknownServer)
    ));
    assert!(matches!(
        registry.connect_ws(&unknown, "/api/x").await,
        Err(RemoteError::UnknownServer)
    ));
}

#[tokio::test]
async fn unreachable_remote_is_not_added() {
    let registry = Registry::new(ServerStore::memory());
    let changes = registry.subscribe();
    let dead = NewServer {
        name: "Dead".into(),
        // Port 9 (discard) on loopback: refused immediately.
        url: "http://127.0.0.1:9".into(),
        username: None,
        password: None,
    };
    let err = registry.add(dead).await.unwrap_err();
    assert!(matches!(err, RemoteError::Unreachable(_)), "{err:?}");
    assert!(registry.servers().await.is_empty());
    assert!(!changes.has_changed().unwrap());
}

#[tokio::test]
async fn loads_existing_store_and_probes_unreachable() {
    let dir = tempfile::tempdir().unwrap();
    let store = ServerStore::at(dir.path().join("servers.json"));
    store
        .save(&[ServerRecord {
            id: ServerId::parse("dead").unwrap(),
            name: "Dead".into(),
            url: ServerUrl::parse("http://127.0.0.1:9").unwrap(),
            credentials: Credentials::Open,
        }])
        .unwrap();
    let registry = Registry::new(store);
    let servers = registry.servers().await;
    assert_eq!(servers.len(), 1);
    let statuses = registry.statuses().await;
    assert_eq!(statuses[0].1, ServerStatus::Unreachable);
}
