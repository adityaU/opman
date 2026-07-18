use super::*;
use serde_json::json;

#[test]
fn client_presence_roundtrip_with_focus() {
    let p = ClientPresence {
        client_id: "c1".into(),
        interface_type: "web".into(),
        focused_session: Some("s1".into()),
        last_seen: "2026-01-01T00:00:00Z".into(),
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["client_id"], "c1");
    assert_eq!(v["interface_type"], "web");
    assert_eq!(v["focused_session"], "s1");
    let back: ClientPresence = serde_json::from_value(v).unwrap();
    assert_eq!(back.client_id, "c1");
    assert_eq!(back.focused_session.as_deref(), Some("s1"));
    assert!(format!("{:?}", back.clone()).contains("ClientPresence"));
}

#[test]
fn client_presence_skips_none_focus() {
    let p = ClientPresence {
        client_id: "c".into(),
        interface_type: "tui".into(),
        focused_session: None,
        last_seen: "t".into(),
    };
    let v = serde_json::to_value(&p).unwrap();
    assert!(v.get("focused_session").is_none());
    let back: ClientPresence = serde_json::from_value(v).unwrap();
    assert!(back.focused_session.is_none());
}

#[test]
fn presence_snapshot_serialize() {
    let snap = PresenceSnapshot {
        clients: vec![ClientPresence {
            client_id: "c".into(),
            interface_type: "web".into(),
            focused_session: None,
            last_seen: "t".into(),
        }],
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["clients"][0]["client_id"], "c");
    assert!(format!("{:?}", snap.clone()).contains("PresenceSnapshot"));
}

#[test]
fn presence_register_request_full_and_default() {
    let full: PresenceRegisterRequest = serde_json::from_value(json!({
        "client_id": "c",
        "interface_type": "web",
        "focused_session": "s"
    }))
    .unwrap();
    assert_eq!(full.client_id, "c");
    assert_eq!(full.focused_session.as_deref(), Some("s"));

    let minimal: PresenceRegisterRequest = serde_json::from_value(json!({
        "client_id": "c",
        "interface_type": "tui"
    }))
    .unwrap();
    assert!(minimal.focused_session.is_none());
    assert!(format!("{:?}", minimal.clone()).contains("PresenceRegisterRequest"));
}

#[test]
fn presence_deregister_request() {
    let req: PresenceDeregisterRequest = serde_json::from_value(json!({"client_id": "c"})).unwrap();
    assert_eq!(req.client_id, "c");
    assert!(format!("{:?}", req.clone()).contains("PresenceDeregisterRequest"));
}

#[test]
fn presence_response_serialize() {
    let resp = PresenceResponse { clients: vec![] };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v["clients"].as_array().unwrap().is_empty());
    assert!(format!("{:?}", resp.clone()).contains("PresenceResponse"));
}
