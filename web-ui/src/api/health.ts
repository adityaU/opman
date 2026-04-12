/** Process health API — matches backend /health/* endpoints. */

import { apiFetch, apiPost } from "./client";

// ── Types ────────────────────────────────────────────────────────

export interface MitigationConfig {
  orphan_cleanup: boolean;
  port_cleanup: boolean;
  temp_cleanup: boolean;
  fd_watchdog: boolean;
  memory_watchdog: boolean;
  connection_watchdog: boolean;
}

export interface MitigationInfo {
  id: string;
  label: string;
  enabled: boolean;
}

export interface PortRecord {
  port: number;
  pid: number;
  state: string;
}

export interface HealthSnapshot {
  orphan_pids: number[];
  tracked_ports: PortRecord[];
  tracked_temp_files: string[];
  open_fds: number | null;
  fd_limit: number | null;
  memory_rss_bytes: number | null;
  tcp_connections: number | null;
}

export interface HealthStatusResponse {
  config: MitigationConfig;
  snapshot: HealthSnapshot;
  mitigations: MitigationInfo[];
}

export interface AuditEntry {
  timestamp: string;
  mitigation: string;
  action: string;
  detail: string;
  success: boolean;
}

export interface HealthAuditResponse {
  entries: AuditEntry[];
}

// ── API calls ────────────────────────────────────────────────────

/** Fetch current health status (config + snapshot + mitigation list). */
export function fetchHealthStatus(): Promise<HealthStatusResponse> {
  return apiFetch<HealthStatusResponse>("/health/status");
}

/** Fetch recent audit log entries. */
export function fetchHealthAudit(limit: number): Promise<HealthAuditResponse> {
  return apiFetch<HealthAuditResponse>(`/health/audit?limit=${limit}`);
}

/** Toggle a single mitigation on/off. Returns updated status. */
export function toggleMitigation(
  mitigation: string,
  enabled: boolean,
): Promise<HealthStatusResponse> {
  return apiPost<HealthStatusResponse>("/health/toggle", { mitigation, enabled });
}
