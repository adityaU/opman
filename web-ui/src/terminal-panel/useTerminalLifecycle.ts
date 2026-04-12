import React, { useRef, useEffect, useCallback, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { spawnPty, ptyWrite, ptyResize, ptyKill, createPtySSE } from "../api";
import {
  TabInfo,
  TabRuntime,
  PtyKind,
  TERM_OPTIONS,
  getTerminalTheme,
} from "./types";

// ── Terminal lifecycle hook ────────────────────────────

export function useTerminalLifecycle(
  tabs: TabInfo[],
  setTabs: React.Dispatch<React.SetStateAction<TabInfo[]>>,
  sessionId: string | null,
  runtimesRef: React.MutableRefObject<Map<string, TabRuntime>>,
  containerRefs: React.MutableRefObject<Map<string, HTMLDivElement>>,
  activeTabId: string | null,
  expanded: boolean,
  visible: boolean,
  createTab: (kind: PtyKind) => void
) {
  // Auto-create first tab on mount
  const initializedRef = useRef(false);
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      createTab("shell");
    }
  }, [createTab]);

  // Track pending tabs whose container refs weren't available yet
  const pendingRef = useRef<Set<string>>(new Set());

  // Initialize xterm for each tab
  useEffect(() => {
    let pendingRetry = false;

    for (const tab of tabs) {
      if (runtimesRef.current.has(tab.id)) continue;
      const container = containerRefs.current.get(tab.id);
      if (!container) {
        // Container not in DOM yet — mark as pending for retry
        pendingRef.current.add(tab.id);
        pendingRetry = true;
        continue;
      }

      pendingRef.current.delete(tab.id);

      const term = new Terminal({ ...TERM_OPTIONS, theme: getTerminalTheme() });
      const fitAddon = new FitAddon();
      const webLinksAddon = new WebLinksAddon();
      const searchAddon = new SearchAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(webLinksAddon);
      term.loadAddon(searchAddon);

      term.open(container);
      fitAddon.fit();

      const ptyId = tab.id;
      term.onData((data: string) => {
        const encoded = btoa(
          Array.from(new TextEncoder().encode(data), (b) =>
            String.fromCharCode(b)
          ).join("")
        );
        ptyWrite(ptyId, encoded).catch(() => {});
      });

      term.onResize((dims: { cols: number; rows: number }) => {
        ptyResize(ptyId, dims.rows, dims.cols).catch(() => {});
      });

      const observer = new ResizeObserver(() => fitAddon.fit());
      observer.observe(container);

      const runtime: TabRuntime = {
        term,
        fit: fitAddon,
        search: searchAddon,
        sse: null,
        observer,
        container,
      };
      runtimesRef.current.set(tab.id, runtime);

      const rows = term.rows;
      const cols = term.cols;
      const sid = tab.kind === "opencode" ? sessionId ?? undefined : undefined;

      spawnPty(tab.kind, tab.id, rows, cols, sid)
        .then(() => {
          setTabs((prev) =>
            prev.map((t) => (t.id === tab.id ? { ...t, status: "ready" } : t))
          );
          const sse = createPtySSE(tab.id);
          runtime.sse = sse;
          sse.addEventListener("output", (event: MessageEvent) => {
            try {
              const raw = atob(event.data);
              const bytes = new Uint8Array(raw.length);
              for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
              term.write(bytes);
            } catch {
              term.write(event.data);
            }
          });
        })
        .catch((err) => {
          console.error(`Failed to spawn PTY for tab ${tab.id}:`, err);
          setTabs((prev) =>
            prev.map((t) => (t.id === tab.id ? { ...t, status: "error" } : t))
          );
          term.write("\r\n\x1b[31mFailed to spawn terminal process.\x1b[0m\r\n");
        });
    }

    // If any tabs were pending (container not yet rendered), retry after
    // the next animation frame so React has time to commit the DOM.
    if (pendingRetry) {
      const raf = requestAnimationFrame(() => {
        // Trigger a re-check by doing a harmless state poke
        setTabs((prev) => [...prev]);
      });
      return () => cancelAnimationFrame(raf);
    }
  }, [tabs, sessionId, runtimesRef, containerRefs, setTabs]);

  // Re-fit active tab on switch/expand/visibility change
  useEffect(() => {
    if (!activeTabId || !visible) return;
    const rt = runtimesRef.current.get(activeTabId);
    if (rt) {
      requestAnimationFrame(() => rt.fit.fit());
    }
  }, [activeTabId, expanded, visible, runtimesRef]);

  // Cleanup on unmount
  useEffect(() => {
    const runtimes = runtimesRef.current;
    return () => {
      for (const [tabId, rt] of runtimes) {
        rt.observer?.disconnect();
        rt.sse?.close();
        rt.term.dispose();
        ptyKill(tabId).catch(() => {});
      }
      runtimes.clear();
    };
  }, [runtimesRef]);
}

// ── Search hook ────────────────────────────────────────

export function useTerminalSearch(
  activeTabId: string | null,
  runtimesRef: React.MutableRefObject<Map<string, TabRuntime>>
) {
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);

  const handleSearchChange = useCallback(
    (query: string) => {
      setSearchQuery(query);
      if (!activeTabId) return;
      const rt = runtimesRef.current.get(activeTabId);
      if (rt && query) {
        rt.search.findNext(query, { regex: false, caseSensitive: false });
      }
    },
    [activeTabId, runtimesRef]
  );

  const searchNext = useCallback(() => {
    if (!activeTabId || !searchQuery) return;
    const rt = runtimesRef.current.get(activeTabId);
    rt?.search.findNext(searchQuery, { regex: false, caseSensitive: false });
  }, [activeTabId, searchQuery, runtimesRef]);

  const searchPrev = useCallback(() => {
    if (!activeTabId || !searchQuery) return;
    const rt = runtimesRef.current.get(activeTabId);
    rt?.search.findPrevious(searchQuery, { regex: false, caseSensitive: false });
  }, [activeTabId, searchQuery, runtimesRef]);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchQuery("");
    if (activeTabId) {
      const rt = runtimesRef.current.get(activeTabId);
      rt?.search.clearDecorations();
      rt?.term.focus();
    }
  }, [activeTabId, runtimesRef]);

  // Ctrl+F / Cmd+F keyboard shortcut
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        const termPanel = document.querySelector(".terminal-panel");
        if (!termPanel) return;
        if (termPanel.contains(document.activeElement) || searchOpen) {
          e.preventDefault();
          e.stopPropagation();
          if (searchOpen) {
            closeSearch();
          } else {
            setSearchOpen(true);
            requestAnimationFrame(() => searchInputRef.current?.focus());
          }
        }
      }
    };
    document.addEventListener("keydown", handler, true);
    return () => document.removeEventListener("keydown", handler, true);
  }, [searchOpen, closeSearch]);

  return {
    searchOpen,
    setSearchOpen,
    searchQuery,
    searchInputRef,
    handleSearchChange,
    searchNext,
    searchPrev,
    closeSearch,
  };
}
