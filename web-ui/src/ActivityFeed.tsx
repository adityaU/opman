import React, { useState, useEffect, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { fetchActivityFeed } from "./api";
import type { ActivityEvent } from "./api";
import { X, FileCode, Terminal, Shield, HelpCircle, Activity, Zap } from "lucide-react";
import { semanticEventColors } from "./utils/theme";

interface Props {
  sessionId: string | null;
  onClose: () => void;
  /** Live activity events pushed from SSE (newest first in the array). */
  liveEvents?: ActivityEvent[];
}

const KIND_CONFIG: Record<string, { color: string; icon: React.ReactNode; label: string }> = {
  file_edit: { color: semanticEventColors.file_edit, icon: <FileCode size={12} />, label: "File Edit" },
  tool_call: { color: semanticEventColors.tool_call, icon: <Zap size={12} />, label: "Tool Call" },
  terminal: { color: semanticEventColors.terminal, icon: <Terminal size={12} />, label: "Terminal" },
  permission: { color: semanticEventColors.permission, icon: <Shield size={12} />, label: "Permission" },
  question: { color: semanticEventColors.question, icon: <HelpCircle size={12} />, label: "Question" },
  status: { color: semanticEventColors.status, icon: <Activity size={12} />, label: "Status" },
};

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return "";
  }
}

function relativeTime(iso: string): string {
  try {
    const d = new Date(iso);
    const diff = (Date.now() - d.getTime()) / 1000;
    if (diff < 60) return `${Math.round(diff)}s ago`;
    if (diff < 3600) return `${Math.round(diff / 60)}m ago`;
    return `${Math.round(diff / 3600)}h ago`;
  } catch {
    return "";
  }
}

export function ActivityFeed({ sessionId, onClose, liveEvents }: Props) {
  const [events, setEvents] = useState<ActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoScroll, setAutoScroll] = useState(true);
  const [hovering, setHovering] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // Fetch initial events
  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;
    setLoading(true);
    fetchActivityFeed(sessionId)
      .then((resp) => {
        if (!cancelled) setEvents(resp.events);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [sessionId]);

  // Merge live events
  useEffect(() => {
    if (!liveEvents?.length) return;
    setEvents((prev) => {
      const existingIds = new Set(prev.map((e) => `${e.timestamp}-${e.kind}-${e.summary}`));
      const newOnes = liveEvents.filter((e) => !existingIds.has(`${e.timestamp}-${e.kind}-${e.summary}`));
      if (!newOnes.length) return prev;
      return [...prev, ...newOnes];
    });
  }, [liveEvents]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoScroll && !hovering && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events, autoScroll, hovering]);

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const el = scrollRef.current;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    setAutoScroll(atBottom);
  }, []);

  return (
    <div className="activity-feed-overlay" onClick={onClose}>
      <div
        className="activity-feed-panel"
        ref={modalRef}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="activity-feed-header">
          <h3>Activity Feed</h3>
          <div className="activity-feed-header-right">
            {!autoScroll && (
              <button
                className="activity-feed-scroll-btn"
                onClick={() => {
                  setAutoScroll(true);
                  if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
                }}
              >
                Scroll to bottom
              </button>
            )}
            <button onClick={onClose} aria-label="Close">
              <X size={14} />
            </button>
          </div>
        </div>

        <div
          className="activity-feed-body"
          ref={scrollRef}
          onScroll={handleScroll}
          onMouseEnter={() => setHovering(true)}
          onMouseLeave={() => setHovering(false)}
        >
          {loading && <div className="activity-feed-empty">Loading activity...</div>}
          {!loading && events.length === 0 && (
            <div className="activity-feed-empty">No activity yet for this session.</div>
          )}
          {events.map((ev, i) => {
            const cfg = KIND_CONFIG[ev.kind] || KIND_CONFIG.status;
            return (
              <div key={i} className="activity-feed-event">
                <span
                  className="activity-feed-event-icon"
                  style={{ color: cfg.color }}
                  title={cfg.label}
                >
                  {cfg.icon}
                </span>
                <span className="activity-feed-event-time" title={formatTime(ev.timestamp)}>
                  {relativeTime(ev.timestamp)}
                </span>
                <span className="activity-feed-event-summary">{ev.summary}</span>
                {ev.detail && (
                  <span className="activity-feed-event-detail" title={ev.detail}>
                    {ev.detail}
                  </span>
                )}
              </div>
            );
          })}
        </div>

        <div className="activity-feed-footer">
          <span>{events.length} event{events.length !== 1 ? "s" : ""}</span>
          <span className="activity-feed-legend">
            {Object.entries(KIND_CONFIG).map(([key, cfg]) => (
              <span key={key} className="activity-feed-legend-item" style={{ color: cfg.color }}>
                {cfg.icon} {cfg.label}
              </span>
            ))}
          </span>
        </div>
      </div>
    </div>
  );
}
