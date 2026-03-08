import React, { useState, useEffect, useRef, useCallback } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Search, ExternalLink, Clock, User, Bot, Loader } from "lucide-react";
import { searchMessages } from "./api";
import type { SearchResultEntry } from "./api";

interface Props {
  onClose: () => void;
  projectIdx: number;
  /** Navigate to a session+message. Parent should switch session. */
  onNavigate: (sessionId: string) => void;
}

/**
 * Cross-session search modal — searches all sessions in the current project
 * via the backend /api/project/{idx}/search endpoint.
 * Triggered by Cmd+Shift+F.
 */
export function CrossSessionSearchModal({ onClose, projectIdx, onNavigate }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResultEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Debounced search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);

    if (!query.trim()) {
      setResults([]);
      setTotal(0);
      setError(null);
      return;
    }

    debounceRef.current = setTimeout(async () => {
      setLoading(true);
      setError(null);
      try {
        const resp = await searchMessages(projectIdx, query.trim(), 50);
        setResults(resp.results);
        setTotal(resp.total);
        setSelectedIndex(0);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Search failed");
        setResults([]);
        setTotal(0);
      } finally {
        setLoading(false);
      }
    }, 300);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, projectIdx]);

  const handleNavigate = useCallback(
    (entry: SearchResultEntry) => {
      onNavigate(entry.session_id);
      onClose();
    },
    [onNavigate, onClose]
  );

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (results[selectedIndex]) {
        handleNavigate(results[selectedIndex]);
      }
    }
  };

  // Group results by session for display
  const grouped = groupBySession(results);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div ref={modalRef} className="cross-session-search-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="cross-session-search-header">
          <Search size={16} className="cross-session-search-icon" />
          <input
            ref={inputRef}
            className="cross-session-search-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search across all sessions..."
          />
          {loading && <Loader size={14} className="cross-session-search-spinner" />}
        </div>

        <div className="cross-session-search-body">
          {error && <div className="cross-session-search-error">{error}</div>}

          {!query.trim() && (
            <div className="cross-session-search-hint">
              Type to search message content, tool calls, and outputs across all sessions in this project.
            </div>
          )}

          {query.trim() && !loading && results.length === 0 && !error && (
            <div className="cross-session-search-empty">
              No results found for "{query}"
            </div>
          )}

          {grouped.map((group) => (
            <div key={group.sessionId} className="cross-session-search-group">
              <div className="cross-session-search-group-header">
                <span className="cross-session-search-session-title">
                  {group.sessionTitle}
                </span>
                <span className="cross-session-search-match-count">
                  {group.entries.length} match{group.entries.length !== 1 ? "es" : ""}
                </span>
              </div>
              {group.entries.map((entry, idx) => {
                const globalIdx = results.indexOf(entry);
                return (
                  <button
                    key={`${entry.session_id}-${entry.message_id}-${idx}`}
                    className={`cross-session-search-result ${globalIdx === selectedIndex ? "selected" : ""}`}
                    onClick={() => handleNavigate(entry)}
                    onMouseEnter={() => setSelectedIndex(globalIdx)}
                  >
                    <div className="cross-session-search-result-meta">
                      {entry.role === "user" ? <User size={12} /> : <Bot size={12} />}
                      <span className="cross-session-search-role">{entry.role}</span>
                      <Clock size={10} />
                      <span className="cross-session-search-time">
                        {formatTimestamp(entry.timestamp)}
                      </span>
                    </div>
                    <div className="cross-session-search-snippet">
                      <HighlightedSnippet text={entry.snippet} query={query} />
                    </div>
                    <ExternalLink size={10} className="cross-session-search-go" />
                  </button>
                );
              })}
            </div>
          ))}

          {total > results.length && (
            <div className="cross-session-search-more">
              Showing {results.length} of {total} results
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────────────────

interface SessionGroup {
  sessionId: string;
  sessionTitle: string;
  entries: SearchResultEntry[];
}

function groupBySession(results: SearchResultEntry[]): SessionGroup[] {
  const map = new Map<string, SessionGroup>();
  for (const entry of results) {
    let group = map.get(entry.session_id);
    if (!group) {
      group = {
        sessionId: entry.session_id,
        sessionTitle: entry.session_title || entry.session_id.slice(0, 8),
        entries: [],
      };
      map.set(entry.session_id, group);
    }
    group.entries.push(entry);
  }
  return Array.from(map.values());
}

function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" }) +
    " " + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** Simple inline highlight of query matches within text */
function HighlightedSnippet({ text, query }: { text: string; query: string }) {
  if (!query.trim()) return <>{text}</>;
  const parts: React.ReactNode[] = [];
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let lastIdx = 0;
  let idx = lowerText.indexOf(lowerQuery, 0);
  let key = 0;
  while (idx !== -1) {
    if (idx > lastIdx) parts.push(text.slice(lastIdx, idx));
    parts.push(
      <mark key={key++} className="search-highlight">
        {text.slice(idx, idx + query.length)}
      </mark>
    );
    lastIdx = idx + query.length;
    idx = lowerText.indexOf(lowerQuery, lastIdx);
  }
  if (lastIdx < text.length) parts.push(text.slice(lastIdx));
  return <>{parts}</>;
}
