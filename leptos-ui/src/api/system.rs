//! System API — matches React `api/system.ts`.
//! SSE stream for system stats is handled by the SSE module, not here.

use crate::types::api::SystemStats;
use super::client::{api_fetch, ApiError};

/// Fetch a snapshot of system stats (one-shot, non-streaming).
pub async fn fetch_system_stats() -> Result<SystemStats, ApiError> {
    api_fetch("/system/stats").await
}
