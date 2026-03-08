import React, { useState, useEffect, useCallback, useMemo, useRef } from "react";
import {
  Loader2,
  X,
  Activity,
  DollarSign,
  Clock,
  Circle,
  StopCircle,
  RefreshCw,
} from "lucide-react";
import { fetchSessionsOverview, abortSession } from "./api";
import type { SessionOverviewEntry, SessionsOverviewResponse } from "./api";
import { useFocusTrap } from "./hooks/useFocusTrap";

interface SessionDashboardProps {
  /** Navigate to a session when clicked */
  onSelectSession: (projectIndex: number, sessionId: string) => void;
  /** Close the dashboard */
  onClose: () => void;
  /** Currently active session ID (for highlighting) */
  activeSessionId: string | null;
}

type SortMode = "activity" | "cost" | "project";

/** Format a Unix timestamp as relative time */
function relativeTime(ts: number): string {
  const diff = Date.now() / 1000 - ts;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

/** Format cost as $X.XX */
function formatCost(cost: number): string {
  return `$${cost.toFixed(2)}`;
}

export function SessionDashboard({
  onSelectSession,
  onClose,
  activeSessionId,
}: SessionDashboardProps) {
  const [data, setData] = useState<SessionsOverviewResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<SortMode>("activity");
  const [aborting, setAborting] = useState<Set<string>>(new Set());
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      const resp = await fetchSessionsOverview();
      setData(resp);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load sessions");
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch on mount + auto-refresh every 5s
  useEffect(() => {
    load();
    timerRef.current = setInterval(load, 5000);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [load]);

  // Escape to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  useFocusTrap(modalRef);

  const handleAbort = useCallback(
    async (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      setAborting((prev) => new Set(prev).add(sessionId));
      try {
        await abortSession(sessionId);
        await load();
      } finally {
        setAborting((prev) => {
          const next = new Set(prev);
          next.delete(sessionId);
          return next;
        });
      }
    },
    [load]
  );

  const sorted = useMemo(() => {
    if (!data) return [];
    const sessions = [...data.sessions];
    switch (sortBy) {
      case "activity":
        sessions.sort((a, b) => b.time.updated - a.time.updated);
        break;
      case "cost":
        sessions.sort(
          (a, b) => (b.stats?.cost ?? 0) - (a.stats?.cost ?? 0)
        );
        break;
      case "project":
        sessions.sort(
          (a, b) =>
            a.project_name.localeCompare(b.project_name) ||
            b.time.updated - a.time.updated
        );
        break;
    }
    return sessions;
  }, [data, sortBy]);

  const totalCost = useMemo(
    () =>
      data
        ? data.sessions.reduce((sum, s) => sum + (s.stats?.cost ?? 0), 0)
        : 0,
    [data]
  );

  return (
    <div className="session-dashboard-overlay" onClick={onClose}>
      <div
        className="session-dashboard-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Session Dashboard"
        ref={modalRef}
      >
        {/* Header */}
        <div className="session-dashboard-header">
          <h2>Session Dashboard</h2>
          <div className="session-dashboard-stats">
            <span>
              <Activity size={13} />
              {data ? data.total : "–"} sessions
            </span>
            <span>
              <Circle size={13} />
              {data ? data.busy_count : "–"} busy
            </span>
          </div>
          <button
            className="session-dashboard-refresh"
            onClick={() => {
              setLoading(true);
              load();
            }}
            aria-label="Refresh"
          >
            <RefreshCw size={14} className={loading ? "spin" : ""} />
          </button>
          <button
            className="session-dashboard-close"
            onClick={onClose}
            aria-label="Close dashboard"
          >
            <X size={16} />
          </button>
        </div>

        {/* Sort controls */}
        <div className="session-dashboard-controls">
          <label htmlFor="sd-sort">Sort by</label>
          <select
            id="sd-sort"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortMode)}
          >
            <option value="activity">Recent Activity</option>
            <option value="cost">Cost</option>
            <option value="project">Project</option>
          </select>
        </div>

        {/* Body */}
        {error && <div className="session-dashboard-error">{error}</div>}

        {loading && !data ? (
          <div className="session-dashboard-loading">
            <Loader2 size={20} className="spin" />
            Loading sessions...
          </div>
        ) : (
          <div className="session-dashboard-grid">
            {sorted.map((session) => (
              <SessionCard
                key={session.id}
                session={session}
                isActive={session.id === activeSessionId}
                isAborting={aborting.has(session.id)}
                onSelect={() =>
                  onSelectSession(session.project_index, session.id)
                }
                onAbort={(e) => handleAbort(e, session.id)}
              />
            ))}
            {sorted.length === 0 && (
              <div className="session-dashboard-empty">No sessions found.</div>
            )}
          </div>
        )}

        {/* Footer */}
        <div className="session-dashboard-footer">
          <DollarSign size={13} />
          Total cost: {formatCost(totalCost)}
        </div>
      </div>
    </div>
  );
}

// ── Card sub-component ────────────────────────────────────────────

interface CardProps {
  session: SessionOverviewEntry;
  isActive: boolean;
  isAborting: boolean;
  onSelect: () => void;
  onAbort: (e: React.MouseEvent) => void;
}

function SessionCard({
  session,
  isActive,
  isAborting,
  onSelect,
  onAbort,
}: CardProps) {
  const cls = [
    "session-dashboard-card",
    isActive ? "active" : "",
    session.is_busy ? "busy" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button className={cls} onClick={onSelect}>
      <div className="session-dashboard-card-title">
        <span
          className={`session-dashboard-status-dot ${session.is_busy ? "busy" : "idle"}`}
        />
        <span className="session-dashboard-card-text">
          {session.title || session.id.slice(0, 8)}
        </span>
      </div>

      <span className="session-dashboard-card-project">
        {session.project_name}
      </span>

      <div className="session-dashboard-card-meta">
        <span className="session-dashboard-card-cost">
          <DollarSign size={11} />
          {session.stats ? formatCost(session.stats.cost) : "$0.00"}
        </span>
        <span className="session-dashboard-card-time">
          <Clock size={11} />
          {relativeTime(session.time.updated)}
        </span>
      </div>

      {session.is_busy && (
        <button
          className="session-dashboard-card-stop"
          onClick={onAbort}
          aria-label="Stop session"
          disabled={isAborting}
        >
          {isAborting ? (
            <Loader2 size={12} className="spin" />
          ) : (
            <StopCircle size={12} />
          )}
          Stop
        </button>
      )}
    </button>
  );
}
