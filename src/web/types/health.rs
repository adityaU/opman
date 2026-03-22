//! API types for the process health module.

use serde::{Deserialize, Serialize};

use crate::process_health::{AuditEntry, HealthSnapshot, Mitigation, MitigationConfig};

/// Response for GET /api/health/status.
#[derive(Serialize)]
pub struct HealthStatusResponse {
    pub config: MitigationConfig,
    pub snapshot: HealthSnapshot,
    pub mitigations: Vec<MitigationInfo>,
}

/// Info about a single mitigation for the UI.
#[derive(Serialize)]
pub struct MitigationInfo {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

impl HealthStatusResponse {
    pub fn build(config: &MitigationConfig, snapshot: &HealthSnapshot) -> Self {
        let mitigations = Mitigation::ALL
            .iter()
            .map(|&m| MitigationInfo {
                id: m.as_str().to_string(),
                label: m.label().to_string(),
                enabled: config.is_enabled(m),
            })
            .collect();
        Self {
            config: config.clone(),
            snapshot: snapshot.clone(),
            mitigations,
        }
    }
}

/// Response for GET /api/health/audit.
#[derive(Serialize)]
pub struct HealthAuditResponse {
    pub entries: Vec<AuditEntry>,
}

/// Request body for POST /api/health/toggle.
#[derive(Deserialize)]
pub struct HealthToggleRequest {
    pub mitigation: Mitigation,
    pub enabled: bool,
}

/// Request body for POST /api/health/config.
#[derive(Deserialize)]
pub struct HealthConfigRequest {
    pub config: MitigationConfig,
}
