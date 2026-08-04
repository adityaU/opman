//! Generated coverage tests for `system_handlers.rs`.

use super::*;

use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use axum::extract::State;
use axum::response::IntoResponse;
use sysinfo::{Disks, Networks, System};

#[tokio::test]
async fn get_system_stats_handler_ok() {
    let state = test_server_state();
    let resp = get_system_stats(
        State(state),
        AuthUser {
            subject: "t".into(),
        },
    )
    .await
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // Shape checks — keys must be present.
    assert!(v.get("mem_total").is_some());
    assert!(v.get("cpu_usage").is_some());
    assert!(v.get("hostname").is_some());
    assert!(v.get("processes").is_some());
    assert!(v.get("load_avg").is_some());
}

#[test]
fn collect_system_stats_direct() {
    let stats = collect_system_stats();
    // hostname is populated (or the "unknown" fallback), never empty.
    assert!(!stats.hostname.is_empty());
    // processes are truncated to at most 40.
    assert!(stats.processes.len() <= 40);
    // cpu_avg is finite.
    assert!(stats.cpu_avg.is_finite());
    // process_count >= number of returned (truncated) processes.
    assert!(stats.process_count >= stats.processes.len());
}

#[test]
fn collect_system_stats_reuse_direct() {
    let mut sys = System::new_all();
    sys.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let nets = Networks::new_with_refreshed_list();
    let stats = collect_system_stats_reuse(&sys, &disks, &nets);
    assert!(!stats.hostname.is_empty());
    assert_eq!(stats.cpu_usage.len(), sys.cpus().len());
}

#[test]
fn fallback_stats_all_zero() {
    let s = fallback_stats();
    assert_eq!(s.mem_total, 0);
    assert_eq!(s.mem_used, 0);
    assert_eq!(s.cpu_avg, 0.0);
    assert!(s.cpu_usage.is_empty());
    assert_eq!(s.hostname, "unknown");
    assert_eq!(s.process_count, 0);
    assert!(s.processes.is_empty());
    assert!(s.disks.is_empty());
    assert!(s.networks.is_empty());
    assert_eq!(s.load_avg, [0.0, 0.0, 0.0]);
}
