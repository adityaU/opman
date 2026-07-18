use super::*;
use serde_json::json;

#[test]
fn project_info_deserialize_full() {
    let pi: ProjectInfo =
        serde_json::from_value(json!({ "directory": "/srv", "version": "9.9" })).unwrap();
    assert_eq!(pi.directory, "/srv");
    assert_eq!(pi.version, "9.9");
}

#[test]
fn project_info_deserialize_defaults() {
    // Both fields have #[serde(default)] -> empty object parses to empty strings.
    let pi: ProjectInfo = serde_json::from_value(json!({})).unwrap();
    assert_eq!(pi.directory, "");
    assert_eq!(pi.version, "");
}

#[test]
fn project_info_deserialize_partial() {
    let pi: ProjectInfo = serde_json::from_value(json!({ "version": "1.0" })).unwrap();
    assert_eq!(pi.directory, "");
    assert_eq!(pi.version, "1.0");
}

#[test]
fn project_info_is_clone_and_debug() {
    let pi = ProjectInfo {
        directory: "/d".into(),
        version: "v".into(),
    };
    let cloned = pi.clone();
    assert_eq!(cloned.directory, "/d");
    assert!(format!("{:?}", pi).contains("/d"));
}

#[test]
fn api_client_new_constructs() {
    // Just exercise the constructors — no panics.
    let _c = ApiClient::new();
}

#[test]
fn api_client_with_client_constructs() {
    let shared = reqwest::Client::new();
    let _c = ApiClient::with_client(shared);
}
