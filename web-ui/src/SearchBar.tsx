import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { Search, ChevronUp, ChevronDown, X, Regex } from "lucide-react";
import type { Message } from "./types";

export interface SearchMatch {
  /** Message ID where match was found */
  messageId: string;
  /** Index within the grouped messages for scrolling */
  groupIndex: number;
  /** Matched text snippet (for highlighting) */
  snippet: string;
}

interface Props {
  messages: Message[];
  onClose: () => void;
  /** Called with match message IDs for highlighting in the timeline */
  onMatchesChanged: (matchIds: Set<string>, activeMatchId: string | null) => void;
  /** Called to scroll to a specific group index */
  onScrollToGroup?: (groupIndex: number) => void;
}

/**
 * Extracts all searchable text from a message (role text + tool calls).
 */
function getSearchableText(msg: Message): string {
  const parts: string[] = [];
  for (const part of msg.parts) {
    if (part.text) parts.push(part.text);
    if (part.tool) parts.push(part.tool);
    if (part.toolName) parts.push(part.toolName);
    if (part.args) parts.push(JSON.stringify(part.args));
    if (typeof part.result === "string") parts.push(part.result);
    if (part.state) {
      if (part.state.output) parts.push(part.state.output);
      if (part.state.input) {
        parts.push(
          typeof part.state.input === "string"
            ? part.state.input
            : JSON.stringify(part.state.input)
        );
      }
    }
  }
  return parts.join(" ");
}

/**
 * In-session search bar — shows above the message timeline.
 * Searches message text, tool names, args, and output.
 */
export function SearchBar({ messages, onClose, onMatchesChanged, onScrollToGroup }: Props) {
  const [query, setQuery] = useState("");
  const [useRegex, setUseRegex] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Build group index map: messageId -> group index
  const groupIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    let groupIdx = 0;
    let lastRole: string | null = null;
    for (const msg of messages) {
      if (msg.info.role !== lastRole) {
        if (lastRole !== null) groupIdx++;
        lastRole = msg.info.role;
      }
      const id = msg.info.messageID || msg.info.id || "";
      if (id) map.set(id, groupIdx);
    }
    return map;
  }, [messages]);

  // Compute matches
  const matches: SearchMatch[] = useMemo(() => {
    if (!query.trim()) return [];

    let testFn: (text: string) => boolean;
    if (useRegex) {
      try {
        const re = new RegExp(query, "gi");
        testFn = (text) => re.test(text);
      } catch {
        // Invalid regex — treat as literal
        const lq = query.toLowerCase();
        testFn = (text) => text.toLowerCase().includes(lq);
      }
    } else {
      const lq = query.toLowerCase();
      testFn = (text) => text.toLowerCase().includes(lq);
    }

    const result: SearchMatch[] = [];
    for (const msg of messages) {
      const text = getSearchableText(msg);
      if (testFn(text)) {
        const id = msg.info.messageID || msg.info.id || "";
        // Extract a short snippet around the match
        const idx = text.toLowerCase().indexOf(query.toLowerCase());
        const start = Math.max(0, idx - 30);
        const end = Math.min(text.length, idx + query.length + 30);
        const snippet = (start > 0 ? "..." : "") + text.slice(start, end) + (end < text.length ? "..." : "");
        result.push({
          messageId: id,
          groupIndex: groupIndexMap.get(id) ?? 0,
          snippet,
        });
      }
    }
    return result;
  }, [query, useRegex, messages, groupIndexMap]);

  // Notify parent of match changes
  useEffect(() => {
    const matchIds = new Set(matches.map((m) => m.messageId));
    const activeId = matches[activeIndex]?.messageId ?? null;
    onMatchesChanged(matchIds, activeId);
  }, [matches, activeIndex, onMatchesChanged]);

  // Reset active index when query changes
  useEffect(() => {
    setActiveIndex(0);
  }, [query, useRegex]);

  // Navigate to active match
  useEffect(() => {
    if (matches.length > 0 && onScrollToGroup) {
      onScrollToGroup(matches[activeIndex]?.groupIndex ?? 0);
    }
  }, [activeIndex, matches, onScrollToGroup]);

  const goNext = useCallback(() => {
    if (matches.length === 0) return;
    setActiveIndex((i) => (i + 1) % matches.length);
  }, [matches.length]);

  const goPrev = useCallback(() => {
    if (matches.length === 0) return;
    setActiveIndex((i) => (i - 1 + matches.length) % matches.length);
  }, [matches.length]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.shiftKey) goPrev();
        else goNext();
      }
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [goNext, goPrev, onClose]
  );

  return (
    <div className="search-bar">
      <div className="search-bar-inner">
        <Search size={14} className="search-bar-icon" />
        <input
          ref={inputRef}
          className="search-bar-input"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search in conversation..."
        />
        <button
          className={`search-bar-btn search-bar-regex ${useRegex ? "active" : ""}`}
          onClick={() => setUseRegex((v) => !v)}
          title="Toggle regex"
          aria-label="Toggle regex search"
        >
          <Regex size={14} />
        </button>
        {query && (
          <span className="search-bar-count">
            {matches.length > 0 ? `${activeIndex + 1} of ${matches.length}` : "No matches"}
          </span>
        )}
        <button className="search-bar-btn" onClick={goPrev} disabled={matches.length === 0} title="Previous match" aria-label="Previous match">
          <ChevronUp size={14} />
        </button>
        <button className="search-bar-btn" onClick={goNext} disabled={matches.length === 0} title="Next match" aria-label="Next match">
          <ChevronDown size={14} />
        </button>
        <button className="search-bar-btn" onClick={onClose} title="Close search" aria-label="Close search">
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
