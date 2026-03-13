//! System monitor handler — returns a snapshot of system metrics.

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use sysinfo::{System, Disks, Networks};

use super::super::auth::AuthUser;
use super::super::types::*;

/// GET /api/system/stats — one-shot system metrics snapshot.
///
/// Note: CPU usage will be 0% on the very first request because sysinfo
/// needs two samples to compute a delta. The SSE stream handles this by
/// keeping a persistent System instance.
pub async fn get_system_stats(
    State(_state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let stats = tokio::task::spawn_blocking(collect_system_stats)
        .await
        .unwrap_or_else(|_| fallback_stats());
    Json(stats)
}

/// Collect system stats with a fresh System (one-shot, CPU will be 0% on first call).
pub(crate) fn collect_system_stats() -> SystemStats {
    let mut sys = System::new_all();
    sys.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let networks = Networks::new_with_refreshed_list();
    collect_system_stats_reuse(&sys, &disks, &networks)
}

/// Collect system stats from existing System + Disks + Networks (for SSE reuse).
///
/// This function does NOT call refresh — the caller must have refreshed
/// the System before calling this.
pub(crate) fn collect_system_stats_reuse(
    sys: &System,
    disk_list: &Disks,
    net_list: &Networks,
) -> SystemStats {
    // CPU usage
    let cpu_usage: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_avg = if cpu_usage.is_empty() {
        0.0
    } else {
        cpu_usage.iter().sum::<f32>() / cpu_usage.len() as f32
    };

    // Memory
    let mem_total = sys.total_memory();
    let mem_used = sys.used_memory();
    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();

    // Load average (returns real values on Linux/macOS, zeros on Windows)
    let load = System::load_average();
    let load_avg = [load.one, load.five, load.fifteen];

    // Uptime
    let uptime_secs = System::uptime();

    // Hostname
    let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());

    // Top processes sorted by CPU usage (top 40)
    let mut procs: Vec<ProcessInfo> = sys
        .processes()
        .values()
        .map(|p| {
            let du = p.disk_usage();
            ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu: p.cpu_usage(),
                mem: p.memory(),
                status: format!("{:?}", p.status()),
                disk_read: du.read_bytes,
                disk_write: du.written_bytes,
            }
        })
        .collect();
    procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let process_count = procs.len();
    procs.truncate(40);

    // Disks
    let disks: Vec<DiskInfo> = disk_list
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            mount: d.mount_point().to_string_lossy().to_string(),
            total: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
            fs_type: d.file_system().to_string_lossy().to_string(),
        })
        .collect();

    // Networks
    let networks: Vec<NetworkInfo> = net_list
        .iter()
        .map(|(name, data)| NetworkInfo {
            name: name.to_string(),
            rx_bytes: data.total_received(),
            tx_bytes: data.total_transmitted(),
        })
        .collect();

    SystemStats {
        mem_total,
        mem_used,
        swap_total,
        swap_used,
        cpu_usage,
        cpu_avg,
        uptime_secs,
        hostname,
        load_avg,
        processes: procs,
        process_count,
        disks,
        networks,
    }
}

fn fallback_stats() -> SystemStats {
    SystemStats {
        mem_total: 0,
        mem_used: 0,
        swap_total: 0,
        swap_used: 0,
        cpu_usage: vec![],
        cpu_avg: 0.0,
        uptime_secs: 0,
        hostname: "unknown".to_string(),
        load_avg: [0.0, 0.0, 0.0],
        processes: vec![],
        process_count: 0,
        disks: vec![],
        networks: vec![],
    }
}
