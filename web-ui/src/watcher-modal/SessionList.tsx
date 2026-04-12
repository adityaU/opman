import React from "react";
import { Eye, Loader2 } from "lucide-react";
import type { WatcherSessionEntry } from "../api";
import { groupSessions, GROUP_LABELS, type SessionGroup } from "./types";

interface SessionListProps {
  sessions: WatcherSessionEntry[];
  selectedId: string | null;
  loading: boolean;
  onSelect: (id: string) => void;
}

export function SessionList({
  sessions,
  selectedId,
  loading,
  onSelect,
}: SessionListProps) {
  const grouped = groupSessions(sessions);

  return (
    <div className="watcher-session-list">
      {loading ? (
        <div className="watcher-empty">
          <Loader2 size={16} className="spinning" />
          <span>Loading sessions...</span>
        </div>
      ) : sessions.length === 0 ? (
        <div className="watcher-empty">No sessions available</div>
      ) : (
        (Object.keys(grouped) as SessionGroup[]).map((group) => {
          const items = grouped[group];
          if (items.length === 0) return null;
          return (
            <div key={group} className="watcher-session-group">
              <div className="watcher-group-label">{GROUP_LABELS[group]}</div>
              {items.map((s) => (
                <button
                  key={s.session_id}
                  className={`watcher-session-item ${s.session_id === selectedId ? "selected" : ""}`}
                  onClick={() => onSelect(s.session_id)}
                >
                  <span className="watcher-session-title">
                    {s.title || s.session_id.slice(0, 12)}
                  </span>
                  {s.has_watcher && (
                    <Eye size={11} className="watcher-session-icon" />
                  )}
                  <span className="watcher-session-project">
                    {s.project_name}
                  </span>
                </button>
              ))}
            </div>
          );
        })
      )}
    </div>
  );
}
