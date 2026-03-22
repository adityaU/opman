//! Process health API client — matches backend health endpoints.

use serde::{Deserialize, Serialize};
use super::client::{api_fetch, api_post, ApiError};

// ── Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationConfig {
    pub orphan_cleanup: bool,
    pub port_cleanup: bool,
    pub temp_cleanup: bool,
    pub fd_watchdog: bool,
    pub memory_watchdog: bool,
    pub connection_watchdog: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationInfo {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRecord {
    pub port: u16,
    pub pid: u32,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub orphan_pids: Vec<u32>,
    pub tracked_ports: Vec<PortRecord>,
    pub tracked_temp_files: Vec<String>,
    pub open_fds: Option<u64>,
    pub fd_limit: Option<u64>,
    pub memory_rss_bytes: Option<u64>,
    pub tcp_connections: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusResponse {
    pub config: MitigationConfig,
    pub snapshot: HealthSnapshot,
    pub mitigations: Vec<MitigationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub mitigation: String,
    pub action: String,
    pub detail: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAuditResponse {
    pub entries: Vec<AuditEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToggleRequest {
    pub mitigation: String,
    pub enabled: bool,
}

// ── API calls ───────────────────────────────────────────────────────

/// Fetch current health status (config + snapshot + mitigation list).
pub async fn fetch_health_status() -> Result<HealthStatusResponse, ApiError> {
    api_fetch("/health/status").await
}

/// Fetch recent audit log entries.
pub async fn fetch_health_audit(limit: usize) -> Result<HealthAuditResponse, ApiError> {
    api_fetch(&format!("/health/audit?limit={}", limit)).await
}

/// Toggle a single mitigation on/off.
pub async fn toggle_mitigation(
    mitigation: &str,
    enabled: bool,
) -> Result<HealthStatusResponse, ApiError> {
    api_post(
        "/health/toggle",
        &ToggleRequest {
            mitigation: mitigation.to_string(),
            enabled,
        },
    )
    .await
}
