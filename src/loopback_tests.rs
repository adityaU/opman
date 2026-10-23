use super::*;

fn write(dir: &tempfile::TempDir, body: &str) -> PathBuf {
    let path = dir.path().join("internal.json");
    std::fs::write(&path, body).expect("write descriptor");
    path
}

#[test]
fn load_from_reads_url_and_token() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = write(&dir, r#"{"url":"http://127.0.0.1:7788","token":"abc"}"#);
    let loopback = Loopback::load_from(&path).expect("descriptor parses");
    assert_eq!(loopback.url, "http://127.0.0.1:7788");
    assert_eq!(loopback.token, "abc");
}

#[test]
fn load_from_rejects_incomplete_descriptors() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    assert!(Loopback::load_from(&dir.path().join("missing.json")).is_none());
    assert!(Loopback::load_from(&write(&dir, "not json")).is_none());
    assert!(Loopback::load_from(&write(&dir, r#"{"url":"http://x"}"#)).is_none());
    assert!(Loopback::load_from(&write(&dir, r#"{"token":"t"}"#)).is_none());
    // A non-string url is as unusable as a missing one.
    assert!(Loopback::load_from(&write(&dir, r#"{"url":7,"token":"t"}"#)).is_none());
}

#[test]
fn request_builders_attach_the_shared_token() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = write(&dir, r#"{"url":"http://127.0.0.1:7788","token":"secret"}"#);
    let loopback = Loopback::load_from(&path).expect("descriptor parses");

    for request in [
        loopback.post("/internal/ask").build(),
        loopback.get("/internal/ask").build(),
    ] {
        let request = request.expect("request builds");
        assert_eq!(request.url().as_str(), "http://127.0.0.1:7788/internal/ask");
        assert_eq!(
            request
                .headers()
                .get("x-internal-token")
                .and_then(|v| v.to_str().ok()),
            Some("secret")
        );
    }
}

#[test]
fn descriptor_path_ends_at_the_published_file() {
    let path = descriptor_path().expect("a config dir exists on this platform");
    assert!(path.ends_with("opman/internal.json"), "{path:?}");
}

#[cfg(unix)]
#[test]
fn published_capability_is_private_even_when_replacing_an_old_public_file() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("internal.json");
    std::fs::write(&path, "old token")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
    publish_to(&path, 12345, "new private token")?;
    let mode = std::fs::metadata(&path)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let parsed: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    assert_eq!(parsed["token"], "new private token");
    assert_eq!(std::fs::read_dir(dir.path())?.count(), 1);
    Ok(())
}
