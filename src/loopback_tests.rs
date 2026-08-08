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
