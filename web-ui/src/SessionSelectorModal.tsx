import React, { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Search, Layers, X } from "lucide-react";
import type { ProjectInfo, SessionInfo } from "./api";

interface Props {
  onClose: () => void;
  projects: ProjectInfo[];
  activeSessionId: string | null;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
}

interface FlatSession {
  projectName: string;
  projectIdx: number;
  session: SessionInfo;
  isCurrent: boolean;
}

/** Format a timestamp as relative time */
function relativeTime(ts: number): string {
  const now = Date.now() / 1000;
  const diff = now - ts;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export function SessionSelectorModal({
  onClose,
  projects,
  activeSessionId,
  onSelectSession,
}: Props) {
  const [query, setQuery] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Flatten all sessions across all projects
  const allSessions = useMemo<FlatSession[]>(() => {
    const result: FlatSession[] = [];
    for (const project of projects) {
      for (const session of project.sessions) {
        result.push({
          projectName: project.name,
          projectIdx: project.index,
          session,
          isCurrent: session.id === activeSessionId,
        });
      }
    }
    // Sort by last updated (most recent first)
    result.sort((a, b) => (b.session.time.updated || 0) - (a.session.time.updated || 0));
    return result;
  }, [projects, activeSessionId]);

  // Filter by query
  const filtered = useMemo(() => {
    if (!query) return allSessions;
    const lq = query.toLowerCase();
    return allSessions.filter(
      (s) =>
        s.projectName.toLowerCase().includes(lq) ||
        s.session.title.toLowerCase().includes(lq) ||
        s.session.id.toLowerCase().includes(lq)
    );
  }, [allSessions, query]);

  // Reset selection on filter change
  useEffect(() => {
    setSelectedIdx(0);
  }, [query]);

  // Scroll selected into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIdx] as HTMLElement;
    if (item) item.scrollIntoView({ block: "nearest" });
  }, [selectedIdx]);

  const handleSelect = useCallback(
    (entry: FlatSession) => {
      onSelectSession(entry.session.id, entry.projectIdx);
      onClose();
    },
    [onSelectSession, onClose]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => (i + 1) % Math.max(filtered.length, 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => (i - 1 + filtered.length) % Math.max(filtered.length, 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (filtered[selectedIdx]) {
          handleSelect(filtered[selectedIdx]);
        }
      }
    },
    [filtered, selectedIdx, handleSelect]
  );

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="session-selector-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Select session" ref={modalRef}>
        {/* Header */}
        <div className="session-selector-header">
          <Layers size={14} />
          <span>Select Session</span>
          <span className="session-selector-count">
            {filtered.length} session{filtered.length !== 1 ? "s" : ""}
          </span>
          <button className="session-selector-close" onClick={onClose} aria-label="Close session selector">
            <X size={14} />
          </button>
        </div>

        {/* Search */}
        <div className="session-selector-search">
          <Search size={13} />
          <input
            ref={inputRef}
            className="session-selector-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search sessions across all projects..."
          />
        </div>

        {/* Results */}
        <div className="session-selector-results" ref={listRef}>
          {filtered.length === 0 ? (
            <div className="session-selector-empty">
              {query ? "No matching sessions" : "No sessions found"}
            </div>
          ) : (
            filtered.map((entry, idx) => (
              <button
                key={`${entry.projectIdx}-${entry.session.id}`}
                className={`session-selector-item ${idx === selectedIdx ? "selected" : ""} ${entry.isCurrent ? "current" : ""}`}
                onClick={() => handleSelect(entry)}
                onMouseEnter={() => setSelectedIdx(idx)}
              >
                <div className="session-selector-item-left">
                  <span className="session-selector-project">
                    {entry.projectName}
                  </span>
                  <span className="session-selector-sep">/</span>
                  <span className="session-selector-title">
                    {entry.session.title || entry.session.id.slice(0, 8)}
                  </span>
                  {entry.isCurrent && (
                    <span className="session-selector-badge">current</span>
                  )}
                </div>
                <span className="session-selector-time">
                  {relativeTime(entry.session.time.updated)}
                </span>
              </button>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="session-selector-footer">
          <kbd>Up/Down</kbd> Navigate
          <kbd>Enter</kbd> Select
          <kbd>Esc</kbd> Close
        </div>
      </div>
    </div>
  );
}
