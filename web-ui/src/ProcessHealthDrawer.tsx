/**
 * ProcessHealthDrawer — modal on desktop, bottom sheet on mobile.
 *
 * Shows mitigation toggles, live snapshot metrics, and recent audit log.
 * Rendered via ModalLayer (ModalName "processHealth"), same pattern as CommandPalette.
 */

import React, { useCallback, useEffect, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import type {
  AuditEntry,
  HealthSnapshot,
  HealthStatusResponse,
  MitigationInfo,
} from "./api";
import {
  fetchHealthStatus,
  fetchHealthAudit,
  toggleMitigation,
} from "./api";

// ── Sub-components (inlined — small enough to keep in one file) ──

const MetricCard: React.FC<{ label: string; value: string }> = ({ label, value }) => (
  <div className="health-metric-card">
    <div className="health-metric-value">{value}</div>
    <div className="health-metric-label">{label}</div>
  </div>
);

const SnapshotMetrics: React.FC<{ snapshot: HealthSnapshot }> = ({ snapshot }) => {
  const fdText =
    snapshot.open_fds != null && snapshot.fd_limit != null
      ? `${snapshot.open_fds} / ${snapshot.fd_limit}`
      : "N/A";
  const memText =
    snapshot.memory_rss_bytes != null
      ? `${Math.round(snapshot.memory_rss_bytes / (1024 * 1024))} MB`
      : "N/A";
  const connText = snapshot.tcp_connections != null ? String(snapshot.tcp_connections) : "N/A";

  return (
    <div className="health-metrics">
      <MetricCard label="File Descriptors" value={fdText} />
      <MetricCard label="RSS Memory" value={memText} />
      <MetricCard label="TCP Connections" value={connText} />
      <MetricCard label="Orphan PIDs" value={String(snapshot.orphan_pids.length)} />
      <MetricCard label="Tracked Ports" value={String(snapshot.tracked_ports.length)} />
      <MetricCard label="Temp Files" value={String(snapshot.tracked_temp_files.length)} />
    </div>
  );
};

const MitigationToggle: React.FC<{
  info: MitigationInfo;
  onToggle: (enabled: boolean) => void;
}> = ({ info, onToggle }) => {
  const [enabled, setEnabled] = useState(info.enabled);

  const handleClick = useCallback(() => {
    const next = !enabled;
    setEnabled(next);
    onToggle(next);
  }, [enabled, onToggle]);

  const dotCls = enabled ? "health-dot health-dot-on" : "health-dot health-dot-off";
  const btnCls = enabled
    ? "health-toggle-btn health-toggle-on"
    : "health-toggle-btn health-toggle-off";

  return (
    <div className="health-toggle-row">
      <label className="health-toggle-label">
        <span className={dotCls} />
        <span>{info.label}</span>
      </label>
      <button className={btnCls} onClick={handleClick}>
        {enabled ? "ON" : "OFF"}
      </button>
    </div>
  );
};

const OverviewTab: React.FC<{
  status: HealthStatusResponse | null;
  onToggle: (id: string, enabled: boolean) => void;
}> = ({ status, onToggle }) => {
  if (!status) return <div className="health-empty">No data</div>;

  return (
    <div className="health-overview">
      <div className="health-grid">
        <div className="health-section">
          <div className="health-section-title">Mitigations</div>
          <div className="health-toggles">
            {status.mitigations.map((m) => (
              <MitigationToggle
                key={m.id}
                info={m}
                onToggle={(v) => onToggle(m.id, v)}
              />
            ))}
          </div>
        </div>
        <div className="health-section">
          <div className="health-section-title">Live Metrics</div>
          <SnapshotMetrics snapshot={status.snapshot} />
        </div>
      </div>
    </div>
  );
};

const AuditLogTab: React.FC<{ entries: AuditEntry[] }> = ({ entries }) => {
  if (entries.length === 0) return <div className="health-empty">No audit entries yet</div>;

  return (
    <div className="health-audit">
      <div className="health-audit-list">
        {[...entries].reverse().map((e, i) => {
          const cls = e.success ? "health-audit-entry" : "health-audit-entry health-audit-fail";
          const ts = e.timestamp.slice(0, 19);
          return (
            <div key={i} className={cls}>
              <span className="health-audit-ts">{ts}</span>
              <span className="health-audit-action">{e.action}</span>
              <span className="health-audit-detail">{e.detail}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
};

// ── SVG icon helpers (inline, no dep on an icon library) ─────────

const IconCpu: React.FC<{ size?: number }> = ({ size = 14 }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <rect x="9" y="9" width="6" height="6" />
    <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
  </svg>
);

const IconRefresh: React.FC<{ size?: number }> = ({ size = 12 }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="23 4 23 10 17 10" />
    <polyline points="1 20 1 14 7 14" />
    <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
  </svg>
);

const IconX: React.FC<{ size?: number }> = ({ size = 14 }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
    strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  </svg>
);

// ── Main component ───────────────────────────────────────────────

export interface ProcessHealthDrawerProps {
  onClose: () => void;
}

export const ProcessHealthDrawer: React.FC<ProcessHealthDrawerProps> = ({ onClose }) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<HealthStatusResponse | null>(null);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<"overview" | "audit">("overview");

  useEscape(onClose);
  useFocusTrap(modalRef);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    try {
      const [s, a] = await Promise.all([
        fetchHealthStatus(),
        fetchHealthAudit(100),
      ]);
      setStatus(s);
      setAudit(a.entries);
    } catch {
      // best-effort — keep whatever we already had
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch on mount
  useEffect(() => { fetchAll(); }, [fetchAll]);

  const handleToggle = useCallback(async (mitigationId: string, enabled: boolean) => {
    try {
      const s = await toggleMitigation(mitigationId, enabled);
      setStatus(s);
    } catch {
      // ignore — toggle will visually revert on next refresh
    }
  }, []);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        ref={modalRef}
        className="health-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="health-drawer-header">
          <div className="health-drawer-title">
            <IconCpu />
            <span>Process Health</span>
          </div>
          <div className="health-drawer-tabs">
            <button
              className={tab === "overview" ? "health-tab active" : "health-tab"}
              onClick={() => setTab("overview")}
            >
              Overview
            </button>
            <button
              className={tab === "audit" ? "health-tab active" : "health-tab"}
              onClick={() => setTab("audit")}
            >
              Audit Log
            </button>
          </div>
          <div className="health-drawer-actions">
            <button
              className="health-refresh-btn"
              onClick={fetchAll}
              title="Refresh"
              disabled={loading}
            >
              <IconRefresh />
            </button>
            <button className="health-close-btn" onClick={onClose} aria-label="Close">
              <IconX />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="health-drawer-body">
          {loading && !status ? (
            <div className="health-loading">Loading...</div>
          ) : tab === "audit" ? (
            <AuditLogTab entries={audit} />
          ) : (
            <OverviewTab status={status} onToggle={handleToggle} />
          )}
        </div>
      </div>
    </div>
  );
};
