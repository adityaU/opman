import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { ProjectInfo, SessionInfo } from "./api";
import { useFocusTrap } from "./hooks/useFocusTrap";

const MAX_VISIBLE_RESULTS = 30;

interface Props {
  project: ProjectInfo;
  projectIdx: number;
  onSelect: (projectIdx: number, sessionId: string) => void;
  onClose: () => void;
}

/** Lightweight fuzzy match: returns a score (lower = better) and matched character indices, or null if no match. */
export function fuzzyMatch(query: string, target: string): { score: number; indices: number[] } | null {
  const q = query.toLowerCase();
  const t = target.toLowerCase();

  // Fast path: empty query matches everything
  if (!q) return { score: 0, indices: [] };

  // Try substring match first (highest quality)
  const substringIdx = t.indexOf(q);
  if (substringIdx !== -1) {
    const indices = Array.from({ length: q.length }, (_, i) => substringIdx + i);
    // Bonus for matching at start
    const score = substringIdx === 0 ? 0 : substringIdx;
    return { score, indices };
  }

  // Fuzzy character-by-character match
  let qi = 0;
  const indices: number[] = [];
  let lastMatchIdx = -1;
  let gaps = 0;

  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      if (lastMatchIdx >= 0 && ti - lastMatchIdx > 1) {
        gaps += ti - lastMatchIdx - 1;
      }
      indices.push(ti);
      lastMatchIdx = ti;
      qi++;
    }
  }

  // All query chars must be matched
  if (qi < q.length) return null;

  // Score: prefer fewer gaps, earlier matches, shorter targets
  const score = 100 + gaps * 10 + indices[0] + (t.length - q.length);
  return { score, indices };
}

/** Highlight matched characters in a string */
function HighlightedText({ text, indices }: { text: string; indices: number[] }) {
  if (indices.length === 0) return <>{text}</>;

  const indexSet = new Set(indices);
  const segments: React.ReactNode[] = [];
  let currentRun = "";
  let isHighlighted = false;

  for (let i = 0; i < text.length; i++) {
    const shouldHighlight = indexSet.has(i);
    if (shouldHighlight !== isHighlighted) {
      if (currentRun) {
        segments.push(
          isHighlighted ? (
            <mark key={segments.length} className="search-highlight">{currentRun}</mark>
          ) : (
            <span key={segments.length}>{currentRun}</span>
          )
        );
      }
      currentRun = "";
      isHighlighted = shouldHighlight;
    }
    currentRun += text[i];
  }
  if (currentRun) {
    segments.push(
      isHighlighted ? (
        <mark key={segments.length} className="search-highlight">{currentRun}</mark>
      ) : (
        <span key={segments.length}>{currentRun}</span>
      )
    );
  }

  return <>{segments}</>;
}

export function SessionSearchModal({ project, projectIdx, onSelect, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useFocusTrap(modalRef);

  // Only parent sessions (no subagents)
  const parentSessions = useMemo(() => {
    return project.sessions
      .filter((s) => !s.parentID)
      .sort((a, b) => (b.time.updated || 0) - (a.time.updated || 0));
  }, [project.sessions]);

  // Fuzzy-filtered and scored results
  const results = useMemo(() => {
    if (!query.trim()) {
      return parentSessions.map((s) => ({ session: s, indices: [] as number[] }));
    }

    const scored: { session: SessionInfo; score: number; indices: number[] }[] = [];
    for (const s of parentSessions) {
      const title = s.title || s.id;
      const match = fuzzyMatch(query, title);
      if (match) {
        scored.push({ session: s, score: match.score, indices: match.indices });
      }
    }

    // Sort by score (best matches first)
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, MAX_VISIBLE_RESULTS);
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
            onSelect(projectIdx, results[selected].session.id);
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
      <div className="search-modal" onKeyDown={handleKeyDown} role="dialog" aria-modal="true" aria-label="Search sessions" ref={modalRef}>
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
            placeholder="Fuzzy search sessions..."
            spellCheck={false}
            autoComplete="off"
            aria-label="Search sessions"
          />
        </div>

        <div className="search-modal-separator" />

        <div className="search-modal-results" ref={listRef} role="listbox">
          {results.length === 0 && (
            <div className="search-modal-empty">No matching sessions</div>
          )}
          {results.map((result, i) => (
            <div
              key={result.session.id}
              className={`search-modal-result ${i === selected ? "selected" : ""}`}
              onClick={() => onSelect(projectIdx, result.session.id)}
              onMouseEnter={() => setSelected(i)}
              role="option"
              aria-selected={i === selected}
            >
              <span className="search-result-title">
                <HighlightedText
                  text={result.session.title || result.session.id.slice(0, 12)}
                  indices={result.indices}
                />
              </span>
              <span className="search-result-meta">
                {formatTimestamp(result.session.time.updated)}
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

export function formatTimestamp(ts: number): string {
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
