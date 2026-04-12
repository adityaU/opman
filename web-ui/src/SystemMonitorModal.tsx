import React, { useEffect, useState, useRef, useCallback, useMemo } from "react";
import { Activity, X, Cpu, HardDrive, Network, ChevronDown, ChevronRight, ArrowUpDown } from "lucide-react";
import { connectSystemStatsStream } from "./api/system";
import type { SystemStats, ProcessInfo } from "./api/system";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";

interface Props {
  onClose: () => void;
}

/* ── Sort types ──────────────────────────────────────── */

type SortKey = "pid" | "name" | "cpu" | "mem" | "disk_read" | "disk_write" | "status";
type SortDir = "asc" | "desc";

/* ── Formatting helpers ──────────────────────────────── */

function fmtBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const val = bytes / Math.pow(1024, i);
  return `${val < 10 ? val.toFixed(1) : val.toFixed(0)} ${units[i]}`;
}

function fmtUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const parts: string[] = [];
  if (d > 0) parts.push(`${d}d`);
  if (h > 0) parts.push(`${h}h`);
  parts.push(`${m}m`);
  return parts.join(" ");
}

function pct(used: number, total: number): number {
  return total > 0 ? (used / total) * 100 : 0;
}

function fmtPct(p: number): string {
  return p.toFixed(1) + "%";
}

/* ── Sort comparator ─────────────────────────────────── */

function sortProcesses(
  procs: ProcessInfo[],
  key: SortKey,
  dir: SortDir,
): ProcessInfo[] {
  const sorted = [...procs];
  const mul = dir === "desc" ? -1 : 1;
  sorted.sort((a, b) => {
    let cmp = 0;
    switch (key) {
      case "pid":        cmp = a.pid - b.pid; break;
      case "name":       cmp = a.name.localeCompare(b.name); break;
      case "cpu":        cmp = a.cpu - b.cpu; break;
      case "mem":        cmp = a.mem - b.mem; break;
      case "disk_read":  cmp = a.disk_read - b.disk_read; break;
      case "disk_write": cmp = a.disk_write - b.disk_write; break;
      case "status":     cmp = a.status.localeCompare(b.status); break;
    }
    return cmp * mul;
  });
  return sorted;
}

/* ── Bar color helpers ───────────────────────────────── */

function barLevel(p: number): string {
  if (p >= 90) return "critical";
  if (p >= 70) return "high";
  return "";
}

function cpuCoreLevel(usage: number): string {
  if (usage >= 90) return "sysm-core-burning";
  if (usage >= 60) return "sysm-core-hot";
  return "";
}

function cpuCellClass(cpu: number): string {
  if (cpu >= 80) return "sysm-cell-critical";
  if (cpu >= 30) return "sysm-cell-warn";
  return "";
}

/* ── Column definitions ──────────────────────────────── */

interface ColDef {
  key: SortKey;
  label: string;
  align: "left" | "right";
  width?: number;
  render: (p: ProcessInfo) => React.ReactNode;
}

const COLUMNS: ColDef[] = [
  { key: "pid",        label: "PID",      align: "right", width: 58,  render: (p) => p.pid },
  { key: "name",       label: "Name",     align: "left",              render: (p) => <span title={p.name}>{p.name}</span> },
  { key: "cpu",        label: "CPU",      align: "right", width: 60,  render: (p) => <span className={cpuCellClass(p.cpu)}>{p.cpu.toFixed(1)}%</span> },
  { key: "mem",        label: "Mem",      align: "right", width: 72,  render: (p) => fmtBytes(p.mem) },
  { key: "disk_read",  label: "IO R",     align: "right", width: 72,  render: (p) => fmtBytes(p.disk_read) },
  { key: "disk_write", label: "IO W",     align: "right", width: 72,  render: (p) => fmtBytes(p.disk_write) },
  { key: "status",     label: "State",    align: "left",  width: 58,  render: (p) => p.status },
];

/* ── Main component ──────────────────────────────────── */

export const SystemMonitorModal: React.FC<Props> = ({ onClose }) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState(false);
  const [coresOpen, setCoresOpen] = useState(false);

  // Sort state
  const [sortKey, setSortKey] = useState<SortKey>("cpu");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  // Standard modal hooks
  useEscape(onClose);
  useFocusTrap(modalRef);

  const handleSort = useCallback(
    (key: SortKey) => {
      if (key === sortKey) {
        setSortDir((d) => (d === "desc" ? "asc" : "desc"));
      } else {
        setSortKey(key);
        setSortDir(key === "name" || key === "status" ? "asc" : "desc");
      }
    },
    [sortKey],
  );

  const sortedProcesses = useMemo(
    () => (stats ? sortProcesses(stats.processes, sortKey, sortDir) : []),
    [stats, sortKey, sortDir],
  );

  // Connect SSE
  useEffect(() => {
    const es = connectSystemStatsStream(
      (s) => {
        setStats(s);
        setConnected(true);
        setError(false);
      },
      () => setError(true),
    );
    return () => es.close();
  }, []);

  const memPct = stats ? pct(stats.mem_used, stats.mem_total) : 0;
  const swapPct = stats ? pct(stats.swap_used, stats.swap_total) : 0;

  return (
    <div className="sysm-overlay" onClick={onClose}>
      <div
        className="sysm-modal"
        onClick={(e) => e.stopPropagation()}
        tabIndex={0}
        role="dialog"
        aria-modal="true"
        aria-label="System Monitor"
        ref={modalRef}
      >
        {/* ── Header ──────────────────────────────── */}
        <div className="sysm-header">
          <div className="sysm-header-left">
            <Activity size={16} />
            <h3>System Monitor</h3>
            {stats && (
              <span className="sysm-host">{stats.hostname}</span>
            )}
            {stats && (
              <span className="sysm-badge">up {fmtUptime(stats.uptime_secs)}</span>
            )}
          </div>
          <div className="sysm-header-right">
            <span className="sysm-status">
              <span className={`sysm-dot${error ? " error" : ""}`} />
              {connected ? "Live" : error ? "Error" : "..."}
            </span>
            <button onClick={onClose} aria-label="Close">
              <X size={16} />
            </button>
          </div>
        </div>

        {/* ── Body ────────────────────────────────── */}
        <div className="sysm-body">
          {!stats ? (
            <div className="sysm-empty">Connecting to system monitor...</div>
          ) : (
            <>
              {/* ── Overview gauges ───────────────── */}
              <div className="sysm-gauges">
                <GaugeCard label="CPU" value={fmtPct(stats.cpu_avg)} pct={stats.cpu_avg} color="info" />
                <GaugeCard label="Memory" value={`${fmtBytes(stats.mem_used)} / ${fmtBytes(stats.mem_total)}`} pct={memPct} color="success" />
                {stats.swap_total > 0 && (
                  <GaugeCard label="Swap" value={`${fmtBytes(stats.swap_used)} / ${fmtBytes(stats.swap_total)}`} pct={swapPct} color="warning" />
                )}
              </div>

              {/* ── Stats row (load + process count) ── */}
              <div className="sysm-stats-row">
                <span className="sysm-stat">
                  Load <strong>{stats.load_avg[0].toFixed(2)}</strong> / {stats.load_avg[1].toFixed(2)} / {stats.load_avg[2].toFixed(2)}
                </span>
                <span className="sysm-stat">
                  {stats.process_count} processes
                </span>
                {stats.networks.length > 0 && stats.networks.slice(0, 3).map((n, i) => (
                  <span key={i} className="sysm-stat sysm-stat-net">
                    <Network size={10} />
                    <span className="sysm-net-name">{n.name}</span>
                    <span className="sysm-net-rx">{fmtBytes(n.rx_bytes)}</span>
                    <span className="sysm-net-sep">/</span>
                    <span className="sysm-net-tx">{fmtBytes(n.tx_bytes)}</span>
                  </span>
                ))}
              </div>

              {/* ── CPU cores (collapsible) ────────── */}
              {stats.cpu_usage.length > 1 && (
                <div className="sysm-section">
                  <button
                    className="sysm-section-toggle"
                    onClick={() => setCoresOpen((o) => !o)}
                  >
                    {coresOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                    <Cpu size={11} />
                    <span>CPU Cores ({stats.cpu_usage.length})</span>
                  </button>
                  {coresOpen && (
                    <div className="sysm-cores">
                      {stats.cpu_usage.map((usage, i) => (
                        <div
                          key={i}
                          className={`sysm-core ${cpuCoreLevel(usage)}`}
                          title={`Core #${i}: ${usage.toFixed(1)}%`}
                        >
                          <span className="sysm-core-id">{i}</span>
                          <div className="sysm-core-bar">
                            <div
                              className={`sysm-core-fill ${barLevel(usage)}`}
                              style={{ height: `${Math.min(usage, 100)}%` }}
                            />
                          </div>
                          <span className="sysm-core-val">{usage.toFixed(0)}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* ── Process table ──────────────────── */}
              <div className="sysm-section">
                <div className="sysm-section-label">
                  <ArrowUpDown size={11} />
                  <span>Processes</span>
                </div>
                <div className="sysm-table-wrap">
                  <table className="sysm-table">
                    <thead>
                      <tr>
                        {COLUMNS.map((col) => {
                          const active = sortKey === col.key;
                          const arrow = active
                            ? sortDir === "desc"
                              ? " \u2193"
                              : " \u2191"
                            : "";
                          return (
                            <th
                              key={col.key}
                              className={`${col.align === "right" ? "r" : ""} ${active ? "active" : ""}`}
                              style={col.width ? { width: col.width } : undefined}
                              onClick={() => handleSort(col.key)}
                            >
                              {col.label}{arrow}
                            </th>
                          );
                        })}
                      </tr>
                    </thead>
                    <tbody>
                      {sortedProcesses.map((p) => (
                        <tr key={p.pid}>
                          {COLUMNS.map((col) => (
                            <td
                              key={col.key}
                              className={col.align === "right" ? "r" : ""}
                            >
                              {col.render(p)}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              {/* ── Disks ─────────────────────────── */}
              {stats.disks.length > 0 && (
                <div className="sysm-section">
                  <div className="sysm-section-label">
                    <HardDrive size={11} />
                    <span>Disks</span>
                  </div>
                  <div className="sysm-disks">
                    {stats.disks.map((d, i) => {
                      const dp = pct(d.used, d.total);
                      return (
                        <div key={i} className="sysm-disk">
                          <div className="sysm-disk-top">
                            <span className="sysm-disk-name">{d.mount || d.name}</span>
                            <span className="sysm-disk-pct">{fmtPct(dp)}</span>
                          </div>
                          <div className="sysm-bar-track">
                            <div
                              className={`sysm-bar-fill disk ${barLevel(dp)}`}
                              style={{ width: `${Math.min(dp, 100)}%` }}
                            />
                          </div>
                          <div className="sysm-disk-detail">
                            {fmtBytes(d.used)} / {fmtBytes(d.total)}
                            {d.fs_type && <span className="sysm-disk-fs">{d.fs_type}</span>}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* ── Footer ──────────────────────────────── */}
        <div className="sysm-footer">
          <kbd>Esc</kbd> Close
          <span className="sysm-footer-sep" />
          Click column headers to sort
        </div>
      </div>
    </div>
  );
};

/* ── Gauge card sub-component ────────────────────────── */

const GaugeCard: React.FC<{
  label: string;
  value: string;
  pct: number;
  color: "info" | "success" | "warning" | "error" | "accent";
}> = ({ label, value, pct: p, color }) => (
  <div className="sysm-gauge">
    <div className="sysm-gauge-top">
      <span className="sysm-gauge-label">{label}</span>
      <span className="sysm-gauge-val">{value}</span>
    </div>
    <div className="sysm-bar-track">
      <div
        className={`sysm-bar-fill ${color} ${barLevel(p)}`}
        style={{ width: `${Math.min(p, 100)}%` }}
      />
    </div>
  </div>
);
