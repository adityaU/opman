import React, { useCallback, useEffect, useRef, useState } from "react";
import type { TerminalRuntime } from "./types";

export interface SearchOptions {
  readonly caseSensitive: boolean;
  readonly wholeWord: boolean;
  readonly regex: boolean;
}

export interface SearchResults {
  /** Zero-based index of the current match; -1 when none is current. */
  readonly index: number;
  readonly count: number;
  /** The addon stops counting at its decoration limit. */
  readonly capped: boolean;
}

const MATCH_LIMIT = 1000;

/** Highlight colours must be literal hex for xterm; these read on both light and dark. */
const DECORATIONS = {
  matchBackground: "#7c5cff55",
  matchBorder: "#7c5cff",
  matchOverviewRuler: "#7c5cff",
  activeMatchBackground: "#d29922aa",
  activeMatchBorder: "#d29922",
  activeMatchColorOverviewRuler: "#d29922",
};

/** Find-in-terminal, driven by xterm's own search addon. */
export function useTerminalSearch(
  runtimeRef: React.MutableRefObject<TerminalRuntime | null>,
) {
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [options, setOptions] = useState<SearchOptions>({
    caseSensitive: false,
    wholeWord: false,
    regex: false,
  });
  const [results, setResults] = useState<SearchResults | null>(null);

  const find = useCallback(
    (query: string, backwards: boolean, current: SearchOptions = options) => {
      const runtime = runtimeRef.current;
      if (!runtime) return;
      if (!query) {
        runtime.search.clearDecorations();
        setResults(null);
        return;
      }
      const request = { ...current, decorations: DECORATIONS };
      try {
        if (backwards) runtime.search.findPrevious(query, request);
        else runtime.search.findNext(query, request);
      } catch {
        // A half-typed regular expression is not a pattern yet.
        setResults({ index: -1, count: 0, capped: false });
      }
    },
    [options, runtimeRef],
  );

  const handleSearchChange = useCallback(
    (query: string) => {
      setSearchQuery(query);
      find(query, false);
    },
    [find],
  );

  const toggle = useCallback(
    (option: keyof SearchOptions) => {
      const next = { ...options, [option]: !options[option] };
      setOptions(next);
      find(searchQuery, false, next);
    },
    [find, options, searchQuery],
  );

  // The match count comes from the addon whenever a search runs.
  useEffect(() => {
    if (!searchOpen) return;
    const addon = runtimeRef.current?.search;
    if (!addon) return;
    const listener = addon.onDidChangeResults(({ resultIndex, resultCount }) =>
      setResults({ index: resultIndex, count: resultCount, capped: resultCount >= MATCH_LIMIT }),
    );
    return () => listener.dispose();
  }, [runtimeRef, searchOpen]);

  const searchNext = useCallback(() => find(searchQuery, false), [find, searchQuery]);
  const searchPrev = useCallback(() => find(searchQuery, true), [find, searchQuery]);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchQuery("");
    setResults(null);
    const runtime = runtimeRef.current;
    runtime?.search.clearDecorations();
    runtime?.term.focus();
  }, [runtimeRef]);

  const openSearch = useCallback(() => {
    setSearchOpen(true);
    requestAnimationFrame(() => searchInputRef.current?.focus());
  }, []);

  const toggleSearch = useCallback(() => {
    if (searchOpen) closeSearch();
    else openSearch();
  }, [closeSearch, openSearch, searchOpen]);

  // Ctrl+F / Cmd+F, but only for the terminal the pointer or focus is in.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key !== "f") return;
      const panel = document.querySelector(".terminal-panel");
      if (!panel) return;
      if (!panel.contains(document.activeElement) && !searchOpen) return;
      event.preventDefault();
      event.stopPropagation();
      toggleSearch();
    };
    document.addEventListener("keydown", handler, true);
    return () => document.removeEventListener("keydown", handler, true);
  }, [searchOpen, toggleSearch]);

  return {
    searchOpen,
    searchQuery,
    options,
    results,
    toggle,
    searchInputRef,
    handleSearchChange,
    searchNext,
    searchPrev,
    closeSearch,
    toggleSearch,
  };
}
