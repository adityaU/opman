import React from "react";
import { X, Search, ChevronUp, ChevronDown } from "lucide-react";
import { KeyHint } from "../keybindings/hint/KeyHint";

// ── Search Bar ─────────────────────────────────────────

interface SearchBarProps {
  searchQuery: string;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  onSearchChange: (query: string) => void;
  onSearchNext: () => void;
  onSearchPrev: () => void;
  onClose: () => void;
}

export function SearchBar({
  searchQuery,
  searchInputRef,
  onSearchChange,
  onSearchNext,
  onSearchPrev,
  onClose,
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
