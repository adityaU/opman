use super::*;

fn record(id: &str, credentials: Credentials) -> ServerRecord {
    ServerRecord {
        id: ServerId::parse(id).unwrap(),
        name: format!("Name {id}"),
        url: ServerUrl::parse("http://127.0.0.1:9").unwrap(),
        credentials,
    }
}

#[test]
fn missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = ServerStore::at(dir.path().join("servers.json"));
    assert!(store.load().is_empty());
}

#[test]
fn round_trips_records_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = ServerStore::at(dir.path().join("nested").join("servers.json"));
    let records = [
        record("open", Credentials::Open),
        record(
            "locked",
            Credentials::Password {
                username: "me".into(),
                password: Secret::new("pw"),
            },
        ),
    ];
    store.save(&records).unwrap();
    let loaded = store.load();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id.as_str(), "open");
    assert!(matches!(loaded[0].credentials, Credentials::Open));
    match &loaded[1].credentials {
        Credentials::Password { username, password } => {
            assert_eq!(username, "me");
            assert_eq!(password.expose(), "pw");
        }
        Credentials::Open => panic!("credentials lost"),
    }
}

#[test]
fn file_matches_the_contract_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let store = ServerStore::at(path.clone());
    let locked = Credentials::Password {
        username: "me".into(),
        password: Secret::new("pw"),
    };
    store.save(&[record("dev", locked)]).unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        raw,
        serde_json::json!({ "servers": [{
            "id": "dev", "name": "Name dev", "url": "http://127.0.0.1:9",
            "username": "me", "password": "pw"
        }]})
    );
}

#[test]
fn invalid_entries_are_skipped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let body = serde_json::json!({ "servers": [
        { "id": "BAD ID", "name": "x", "url": "http://h" },
        { "id": "ok", "name": "y", "url": "ftp://h" },
        { "id": "good", "name": "z", "url": "http://h", "username": "" },
    ]});
    std::fs::write(&path, body.to_string()).unwrap();
    let loaded = ServerStore::at(path).load();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id.as_str(), "good");
    assert!(matches!(loaded[0].credentials, Credentials::Open));
}

#[test]
fn garbage_file_loads_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(ServerStore::at(path).load().is_empty());
}

#[test]
fn memory_store_never_touches_disk() {
    let store = ServerStore::memory();
    store.save(&[record("dev", Credentials::Open)]).unwrap();
    assert!(store.load().is_empty());
}

#[cfg(unix)]
#[test]
fn saved_file_is_private_and_open_file_is_tightened() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("servers.json");
    let store = ServerStore::at(path.clone());
    store.save(&[record("dev", Credentials::Open)]).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(store.load().len(), 1);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}
