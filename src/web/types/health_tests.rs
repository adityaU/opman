use super::*;
use crate::process_health::{HealthSnapshot, Mitigation, MitigationConfig, PortRecord};
use serde_json::json;

#[test]
fn health_status_response_build_lists_all_mitigations() {
    let config = MitigationConfig::default();
    let snapshot = HealthSnapshot::default();
    let resp = HealthStatusResponse::build(&config, &snapshot);
    // One MitigationInfo per Mitigation::ALL entry.
    assert_eq!(resp.mitigations.len(), Mitigation::ALL.len());
    // Default config enables everything.
    assert!(resp.mitigations.iter().all(|m| m.enabled));
    // ids/labels match the enum's own strings.
    let ids: Vec<&str> = resp.mitigations.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"orphan_cleanup"));
    assert!(ids.contains(&"connection_watchdog"));
    let labels: Vec<&str> = resp.mitigations.iter().map(|m| m.label.as_str()).collect();
    assert!(labels.contains(&"Orphan Process Cleanup"));

    let v = serde_json::to_value(&resp).unwrap();
    assert!(v["config"].is_object());
    assert!(v["snapshot"].is_object());
    assert_eq!(v["mitigations"].as_array().unwrap().len(), Mitigation::ALL.len());
}

#[test]
fn health_status_response_build_respects_disabled() {
    let mut config = MitigationConfig::default();
    config.set_enabled(Mitigation::OrphanCleanup, false);
    let snapshot = HealthSnapshot {
        orphan_pids: vec![1, 2],
        tracked_ports: vec![PortRecord {
            port: 8080,
            pid: 42,
            state: "LISTEN".into(),
        }],
        tracked_temp_files: vec!["/tmp/x".into()],
        open_fds: Some(10),
        fd_limit: Some(1024),
        memory_rss_bytes: Some(1000),
        tcp_connections: Some(5),
    };
    let resp = HealthStatusResponse::build(&config, &snapshot);
    let orphan = resp
        .mitigations
        .iter()
        .find(|m| m.id == "orphan_cleanup")
        .unwrap();
    assert!(!orphan.enabled);
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["snapshot"]["orphan_pids"], json!([1, 2]));
    assert_eq!(v["snapshot"]["tracked_ports"][0]["port"], 8080);
}

#[test]
fn mitigation_info_serializes() {
    let info = MitigationInfo {
        id: "fd_watchdog".into(),
        label: "File Descriptor Watchdog".into(),
        enabled: false,
    };
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["id"], "fd_watchdog");
    assert_eq!(v["label"], "File Descriptor Watchdog");
    assert_eq!(v["enabled"], false);
}

#[test]
fn health_audit_response_serializes() {
    let resp = HealthAuditResponse { entries: vec![] };
    assert_eq!(serde_json::to_value(&resp).unwrap()["entries"], json!([]));
}

#[test]
fn health_toggle_request_deserializes() {
    let r: HealthToggleRequest = serde_json::from_value(json!({
        "mitigation": "port_cleanup",
        "enabled": true
    }))
    .unwrap();
    assert_eq!(r.mitigation, Mitigation::PortCleanup);
    assert!(r.enabled);
}

#[test]
fn health_config_request_deserializes() {
    let r: HealthConfigRequest = serde_json::from_value(json!({
        "config": {
            "orphan_cleanup": false,
            "port_cleanup": true,
            "temp_cleanup": false,
            "fd_watchdog": true,
            "memory_watchdog": false,
            "connection_watchdog": true
        }
    }))
    .unwrap();
    assert!(!r.config.orphan_cleanup);
    assert!(r.config.port_cleanup);
    assert!(!r.config.memory_watchdog);
}
