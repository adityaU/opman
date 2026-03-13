//! System monitor types for the htop-like panel.

use serde::Serialize;

/// Snapshot of system metrics, sent via SSE to the frontend.
#[derive(Serialize, Clone, Debug)]
pub struct SystemStats {
    /// Total physical memory in bytes.
    pub mem_total: u64,
    /// Used physical memory in bytes.
    pub mem_used: u64,
    /// Total swap in bytes.
    pub swap_total: u64,
    /// Used swap in bytes.
    pub swap_used: u64,
    /// Per-CPU usage percentages (0.0 – 100.0).
    pub cpu_usage: Vec<f32>,
    /// Overall CPU usage percentage (average across all cores).
    pub cpu_avg: f32,
    /// System uptime in seconds.
    pub uptime_secs: u64,
    /// System hostname.
    pub hostname: String,
    /// Load averages (1, 5, 15 min) — Linux/macOS only, empty on Windows.
    pub load_avg: [f64; 3],
    /// Top processes (limited to top 40).
    pub processes: Vec<ProcessInfo>,
    /// Total number of running processes.
    pub process_count: usize,
    /// Disk usage summaries.
    pub disks: Vec<DiskInfo>,
    /// Network interface stats.
    pub networks: Vec<NetworkInfo>,
}

/// Info about a single process.
#[derive(Serialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// CPU usage percentage for this process.
    pub cpu: f32,
    /// Memory usage in bytes.
    pub mem: u64,
    /// Process status string.
    pub status: String,
    /// Disk read bytes (total since process start).
    pub disk_read: u64,
    /// Disk write bytes (total since process start).
    pub disk_write: u64,
}

/// Info about a single disk/mount.
#[derive(Serialize, Clone, Debug)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub fs_type: String,
}

/// Info about a single network interface.
#[derive(Serialize, Clone, Debug)]
pub struct NetworkInfo {
    pub name: String,
    /// Total bytes received.
    pub rx_bytes: u64,
    /// Total bytes transmitted.
    pub tx_bytes: u64,
}
