import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { ProjectInfo, SessionInfo } from "./api";

const MAX_VISIBLE_RESULTS = 20;

interface Props {
  project: ProjectInfo;
  projectIdx: number;
  onSelect: (projectIdx: number, sessionId: string) => void;
  onClose: () => void;
}

export function SessionSearchModal({ project, projectIdx, onSelect, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Only parent sessions (no subagents)
  const parentSessions = useMemo(() => {
    return project.sessions
      .filter((s) => !s.parentID)
      .sort((a, b) => (b.time.updated || 0) - (a.time.updated || 0));
  }, [project.sessions]);

  // Filtered results
  const results = useMemo(() => {
    if (!query.trim()) return parentSessions;
    const q = query.toLowerCase();
    return parentSessions.filter((s) => {
      const title = (s.title || s.id).toLowerCase();
      return title.includes(q);
    });
  }, [parentSessions, query]);

  // Reset selection when results change
  useEffect(() => {
    setSelected(0);
  }, [results.length]);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const items = listRef.current.children;
    if (items[selected]) {
      (items[selected] as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  }, [selected]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          onClose();
          break;
        case "ArrowDown":
          e.preventDefault();
          setSelected((prev) => Math.min(prev + 1, results.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelected((prev) => Math.max(prev - 1, 0));
          break;
        case "Enter":
          e.preventDefault();
          if (results[selected]) {
            onSelect(projectIdx, results[selected].id);
          }
          break;
      }
    },
    [results, selected, projectIdx, onSelect, onClose]
  );

  // Close on backdrop click
  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) {
        onClose();
      }
    },
    [onClose]
  );

  return (
    <div className="search-modal-backdrop" onClick={handleBackdropClick}>
      <div className="search-modal" onKeyDown={handleKeyDown}>
        <div className="search-modal-header">
          <span className="search-modal-title">Search Sessions</span>
          <span className="search-modal-project">{project.name}</span>
        </div>

        <div className="search-modal-input-row">
          <span className="search-modal-prompt">&gt;</span>
          <input
            ref={inputRef}
            className="search-modal-input"
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Type to filter sessions..."
            spellCheck={false}
            autoComplete="off"
          />
        </div>

        <div className="search-modal-separator" />

        <div className="search-modal-results" ref={listRef}>
          {results.length === 0 && (
            <div className="search-modal-empty">No matching sessions</div>
          )}
          {results.map((session, i) => (
            <div
              key={session.id}
              className={`search-modal-result ${i === selected ? "selected" : ""}`}
              onClick={() => onSelect(projectIdx, session.id)}
              onMouseEnter={() => setSelected(i)}
            >
              <span className="search-result-title">
                {session.title || session.id.slice(0, 12)}
              </span>
              <span className="search-result-meta">
                {formatTimestamp(session.time.updated)}
              </span>
            </div>
          ))}
        </div>

        <div className="search-modal-footer">
          <span className="search-modal-hint">
            ↑↓ navigate &nbsp; Enter select &nbsp; Esc cancel
          </span>
          {results.length > 0 && (
            <span className="search-modal-count">
              [{selected + 1}/{results.length}]
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const date = new Date(ts * 1000);
  const now = new Date();
  const diff = now.getTime() - date.getTime();

  if (diff < 60_000) return "just now";
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86400_000) return `${Math.floor(diff / 3600_000)}h ago`;
  if (diff < 604800_000) return `${Math.floor(diff / 86400_000)}d ago`;

  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
