import React, { useCallback, useEffect, useRef, useState } from "react";
import type { TerminalRuntime } from "./types";

/** Find-in-terminal, driven by xterm's own search addon. */
export function useTerminalSearch(
  runtimeRef: React.MutableRefObject<TerminalRuntime | null>,
) {
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);

  const find = useCallback(
    (query: string, backwards: boolean) => {
      if (!query) return;
      const runtime = runtimeRef.current;
      if (!runtime) return;
      const options = { regex: false, caseSensitive: false };
      if (backwards) runtime.search.findPrevious(query, options);
      else runtime.search.findNext(query, options);
    },
    [runtimeRef],
  );

  const handleSearchChange = useCallback(
    (query: string) => {
      setSearchQuery(query);
      find(query, false);
    },
    [find],
  );

  const searchNext = useCallback(() => find(searchQuery, false), [find, searchQuery]);
  const searchPrev = useCallback(() => find(searchQuery, true), [find, searchQuery]);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchQuery("");
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
    searchInputRef,
    handleSearchChange,
    searchNext,
    searchPrev,
    closeSearch,
    toggleSearch,
  };
}
