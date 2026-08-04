//! Coverage for the pure helpers extracted from `collect_system_stats_reuse`
//! (`cpu_average`, `cpu_desc`) and the empty-source branches of the collector that a
//! real, populated `System` never exercises.
use super::*;

use sysinfo::{Disks, Networks, System};

// ── cpu_average ─────────────────────────────────────────────────────

#[test]
fn cpu_average_empty_is_zero() {
    // The `is_empty()` branch — unreachable on a real host (always ≥1 core), so it
    // needs the helper called directly with an empty slice.
    assert_eq!(cpu_average(&[]), 0.0);
}

#[test]
fn cpu_average_single_and_multi() {
    assert_eq!(cpu_average(&[50.0]), 50.0);
    assert_eq!(cpu_average(&[10.0, 20.0, 30.0]), 20.0);
    assert!((cpu_average(&[25.0, 75.0]) - 50.0).abs() < f32::EPSILON);
}

// ── cpu_desc (NaN-safe descending comparator) ───────────────────────

#[test]
fn cpu_desc_orders_descending() {
    // Larger cpu sorts first (a=10,b=90 → Greater means a after b in a desc sort).
    assert_eq!(cpu_desc(10.0, 90.0), std::cmp::Ordering::Greater);
    assert_eq!(cpu_desc(90.0, 10.0), std::cmp::Ordering::Less);
    assert_eq!(cpu_desc(5.0, 5.0), std::cmp::Ordering::Equal);
}

#[test]
fn cpu_desc_nan_is_equal() {
    // The `unwrap_or(Equal)` fallback — any comparison with NaN is unorderable.
    assert_eq!(cpu_desc(f32::NAN, 1.0), std::cmp::Ordering::Equal);
    assert_eq!(cpu_desc(1.0, f32::NAN), std::cmp::Ordering::Equal);
    assert_eq!(cpu_desc(f32::NAN, f32::NAN), std::cmp::Ordering::Equal);
}

#[test]
fn cpu_desc_sorts_a_vec_descending() {
    let mut v = vec![3.0f32, 1.0, 2.0];
    v.sort_by(|a, b| cpu_desc(*a, *b));
    assert_eq!(v, vec![3.0, 2.0, 1.0]);
}

// ── collect_system_stats_reuse with empty sources ───────────────────

#[test]
fn collect_reuse_empty_system_hits_empty_branches() {
    // `System::new()` (no refresh) has no cpus/processes; unrefreshed Disks/Networks
    // are empty — this drives the empty-cpu (cpu_avg 0.0) + empty-iterator arms that a
    // populated host never reaches.
    let sys = System::new();
    let disks = Disks::new();
    let nets = Networks::new();
    let stats = collect_system_stats_reuse(&sys, &disks, &nets);
    assert!(stats.cpu_usage.is_empty());
    assert_eq!(stats.cpu_avg, 0.0);
    assert!(stats.processes.is_empty());
    assert_eq!(stats.process_count, 0);
    assert!(stats.disks.is_empty());
    assert!(stats.networks.is_empty());
    // hostname/uptime/load are still populated from the OS (or their fallbacks).
    assert!(!stats.hostname.is_empty());
}

// ── SystemStats serde round-trips through the handler shape ──────────

#[test]
fn fallback_stats_serializes_with_expected_keys() {
    let s = fallback_stats();
    let v = serde_json::to_value(&s).unwrap();
    for key in [
        "mem_total",
        "mem_used",
        "swap_total",
        "swap_used",
        "cpu_usage",
        "cpu_avg",
        "uptime_secs",
        "hostname",
        "load_avg",
        "processes",
        "process_count",
        "disks",
        "networks",
    ] {
        assert!(v.get(key).is_some(), "missing key {key}");
    }
    assert_eq!(v["hostname"], "unknown");
    assert_eq!(v["load_avg"], serde_json::json!([0.0, 0.0, 0.0]));
}
