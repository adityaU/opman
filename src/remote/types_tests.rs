use super::*;

#[test]
fn id_parse_accepts_slugs_only() {
    assert!(ServerId::parse("dev-box").is_some());
    assert!(ServerId::parse("a1").is_some());
    for bad in ["", "home", "-x", "x-", "Dev", "a b", "a/b", "é", &"a".repeat(33)] {
        assert!(ServerId::parse(bad).is_none(), "{bad:?} should be rejected");
    }
}

#[test]
fn id_derive_slugs_the_name() {
    let id = ServerId::derive("  My Dev Box!! ", |_| false);
    assert_eq!(id.as_str(), "my-dev-box");
}

#[test]
fn id_derive_falls_back_for_symbol_only_names() {
    assert_eq!(ServerId::derive("!!!", |_| false).as_str(), "server");
    assert_eq!(ServerId::derive("日本", |_| false).as_str(), "server");
}

#[test]
fn id_derive_never_yields_home() {
    let id = ServerId::derive("Home", |_| false);
    assert_ne!(id.as_str(), HOME_ID);
    assert!(ServerId::parse(id.as_str()).is_some());
}

#[test]
fn id_derive_suffixes_until_free() {
    let taken = ["dev", "dev-2"];
    let id = ServerId::derive("dev", |id| taken.contains(&id.as_str()));
    assert_eq!(id.as_str(), "dev-3");
}

#[test]
fn id_derive_stays_within_length_when_suffixed() {
    let long = "x".repeat(80);
    let first = ServerId::derive(&long, |_| false);
    let second = ServerId::derive(&long, |id| id == &first);
    assert!(second.as_str().len() <= 32);
    assert!(ServerId::parse(second.as_str()).is_some());
    assert_ne!(first, second);
}

#[test]
fn url_parse_normalises_and_validates() {
    let url = ServerUrl::parse(" http://dev.local:7777/ ").unwrap();
    assert_eq!(url.as_str(), "http://dev.local:7777");
    let nested = ServerUrl::parse("https://host/opman/").unwrap();
    assert_eq!(nested.as_str(), "https://host/opman");
    for bad in ["ftp://x", "not a url", "http://x/?a=1", "http://x/#f", "file:///tmp"] {
        assert!(ServerUrl::parse(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn url_builds_api_paths() {
    let url = ServerUrl::parse("http://h:1").unwrap();
    assert_eq!(url.api("sessions", None), "http://h:1/api/sessions");
    assert_eq!(url.api("/a/b", Some("x=1")), "http://h:1/api/a/b?x=1");
    assert_eq!(url.api("a", Some("")), "http://h:1/api/a");
    let nested = ServerUrl::parse("https://h/base").unwrap();
    assert_eq!(nested.api("auth/login", None), "https://h/base/api/auth/login");
}

#[test]
fn url_builds_ws_urls() {
    let plain = ServerUrl::parse("http://h:1").unwrap();
    assert_eq!(plain.ws("/api/editor/ws?t=1"), "ws://h:1/api/editor/ws?t=1");
    let tls = ServerUrl::parse("https://h/base").unwrap();
    assert_eq!(tls.ws("/api/browser/device"), "wss://h/base/api/browser/device");
}

#[test]
fn status_serialises_snake_case() {
    let json = serde_json::to_string(&[
        ServerStatus::Ok,
        ServerStatus::Unreachable,
        ServerStatus::AuthFailed,
    ])
    .unwrap();
    assert_eq!(json, r#"["ok","unreachable","auth_failed"]"#);
}

#[test]
fn error_maps_to_status() {
    assert_eq!(RemoteError::AuthFailed.status(), ServerStatus::AuthFailed);
    assert_eq!(
        RemoteError::Unreachable("x".into()).status(),
        ServerStatus::Unreachable
    );
}

#[test]
fn debug_never_prints_the_password() {
    let record = ServerRecord {
        id: ServerId::parse("dev").unwrap(),
        name: "Dev".into(),
        url: ServerUrl::parse("http://h").unwrap(),
        credentials: Credentials::Password {
            username: "u".into(),
            password: Secret::new("hunter2"),
        },
    };
    assert!(!format!("{record:?}").contains("hunter2"));
}
