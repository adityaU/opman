import React from "react";
import { X, Search, ChevronUp, ChevronDown } from "lucide-react";
import { KeyHint } from "../keybindings/hint/KeyHint";
import type { SearchOptions, SearchResults } from "./useTerminalSearch";

// ── Search Bar ─────────────────────────────────────────

interface SearchBarProps {
  searchQuery: string;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  onSearchChange: (query: string) => void;
  onSearchNext: () => void;
  onSearchPrev: () => void;
  onClose: () => void;
  options: SearchOptions;
  results: SearchResults | null;
  onToggle: (option: keyof SearchOptions) => void;
}

export function SearchBar({
  searchQuery,
  searchInputRef,
  onSearchChange,
  onSearchNext,
  onSearchPrev,
  onClose,
  options,
  results,
  onToggle,
}: SearchBarProps) {
  return (
    <div className="term-search-bar">
      <Search size={12} className="term-search-icon" />
      <input
        ref={searchInputRef as React.RefObject<HTMLInputElement>}
        className="term-search-input"
        type="text"
        placeholder="Find in terminal..."
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            if (e.shiftKey) onSearchPrev();
            else onSearchNext();
          }
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      />
      <span className="term-search-count" aria-live="polite">
        {searchQuery ? countLabel(results) : ""}
      </span>
      <Toggle label="Match case" text="Aa" on={options.caseSensitive} onClick={() => onToggle("caseSensitive")} />
      <Toggle label="Whole word" text="ab" on={options.wholeWord} onClick={() => onToggle("wholeWord")} />
      <Toggle label="Regular expression" text=".*" on={options.regex} onClick={() => onToggle("regex")} />
      <KeyHint label="Previous match" command="terminal.findPrevious" placement="top">
        <button className="term-search-nav" onClick={onSearchPrev} aria-label="Previous match">
          <ChevronUp size={14} />
        </button>
      </KeyHint>
      <KeyHint label="Next match" command="terminal.findNext" placement="top">
        <button className="term-search-nav" onClick={onSearchNext} aria-label="Next match">
          <ChevronDown size={14} />
        </button>
      </KeyHint>
      <KeyHint label="Close search" chord="Esc" placement="top">
        <button className="term-search-close" onClick={onClose} aria-label="Close search">
          <X size={12} />
        </button>
      </KeyHint>
    </div>
  );
}

function Toggle(props: { label: string; text: string; on: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="term-search-toggle"
      aria-pressed={props.on}
      aria-label={props.label}
      title={props.label}
      onClick={props.onClick}
    >
      {props.text}
    </button>
  );
}

/** "3 of 12", "No results", or nothing while the search has not run. */
function countLabel(results: SearchResults | null): string {
  if (!results) return "";
  if (results.count === 0) return "No results";
  return `${results.index + 1} of ${results.count}${results.capped ? "+" : ""}`;
}
