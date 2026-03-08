import React, {
  useRef,
  useEffect,
  useCallback,
  useState,
  useMemo,
} from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import "@xterm/xterm/css/xterm.css";
import { spawnPty, ptyWrite, ptyResize, ptyKill, createPtySSE } from "./api";
import { X, Maximize2, Minimize2, Plus, Terminal as TermIcon, Search, ChevronUp, ChevronDown } from "lucide-react";

// ── Types ──────────────────────────────────────────────

type PtyKind = "shell" | "neovim" | "git" | "opencode";
type TabStatus = "connecting" | "ready" | "error";

interface TabInfo {
  id: string; // PTY ID (also serves as tab key)
  kind: PtyKind;
  label: string;
  status: TabStatus;
}

interface Props {
  sessionId: string | null;
  onClose: () => void;
  /** Whether the panel is currently visible (used to re-fit xterm on reopen) */
  visible?: boolean;
  /** MCP: whether an AI agent is currently using terminal tools */
  mcpAgentActive?: boolean;
}

// ── Helpers ────────────────────────────────────────────

function uuid(): string {
  return (
    crypto.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
  );
}

const TERM_OPTIONS = {
  cursorBlink: true,
  cursorStyle: "block" as const,
  fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
  fontSize: 13,
  lineHeight: 1.2,
  allowTransparency: true,
  allowProposedApi: true,
};

function getTerminalTheme() {
  const css = getComputedStyle(document.documentElement);
  const text = css.getPropertyValue("--color-text").trim() || "var(--color-text)";
  const muted = css.getPropertyValue("--color-text-muted").trim() || "var(--color-text-muted)";
  const primary = css.getPropertyValue("--color-primary").trim() || "var(--color-primary)";
  const secondary = css.getPropertyValue("--color-secondary").trim() || "var(--color-secondary)";
  const accent = css.getPropertyValue("--color-accent").trim() || "var(--color-accent)";
  const success = css.getPropertyValue("--color-success").trim() || "var(--color-success)";
  const warning = css.getPropertyValue("--color-warning").trim() || "var(--color-warning)";
  const error = css.getPropertyValue("--color-error").trim() || "var(--color-error)";
  const info = css.getPropertyValue("--color-info").trim() || "var(--color-info)";
  const panel = css.getPropertyValue("--color-bg-panel").trim() || "var(--color-bg-panel)";
  return {
    background: "transparent",
    foreground: text,
    cursor: text,
    selectionBackground: `color-mix(in srgb, ${secondary} 28%, transparent)`,
    black: panel,
    red: error,
    green: success,
    yellow: warning,
    blue: secondary,
    magenta: accent,
    cyan: info,
    white: text,
    brightBlack: muted,
    brightRed: error,
    brightGreen: success,
    brightYellow: warning,
    brightBlue: secondary,
    brightMagenta: accent,
    brightCyan: info,
    brightWhite: primary,
    selectionForeground: text,
  };
}

const KIND_LABELS: Record<PtyKind, string> = {
  shell: "Shell",
  neovim: "Neovim",
  git: "Git",
  opencode: "OpenCode",
};

// ── Per-tab runtime (not React state — mutable refs) ───

interface TabRuntime {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  sse: EventSource | null;
  observer: ResizeObserver | null;
  container: HTMLDivElement | null;
}

// ── Component ──────────────────────────────────────────

export function TerminalPanel({ sessionId, onClose, visible = true, mcpAgentActive = false }: Props) {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [kindMenuOpen, setKindMenuOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Close kind menu when clicking outside
  useEffect(() => {
    if (!kindMenuOpen) return;
    const handler = () => setKindMenuOpen(false);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [kindMenuOpen]);

  // Mutable map of per-tab xterm runtime objects
  const runtimesRef = useRef<Map<string, TabRuntime>>(new Map());
  // Container refs keyed by tab ID
  const containerRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  const tabCounter = useRef(0);

  // ── Create a new tab ─────────────────────────────────

  const createTab = useCallback(
    (kind: PtyKind) => {
      tabCounter.current += 1;
      const id = uuid();
      const label = `${KIND_LABELS[kind]} ${tabCounter.current}`;
      const tab: TabInfo = { id, kind, label, status: "connecting" };
      setTabs((prev) => [...prev, tab]);
      setActiveTabId(id);
      setKindMenuOpen(false);
    },
    []
  );

  // ── Close a tab ──────────────────────────────────────

  const closeTab = useCallback(
    (tabId: string) => {
      // Clean up runtime
      const rt = runtimesRef.current.get(tabId);
      if (rt) {
        rt.observer?.disconnect();
        rt.sse?.close();
        rt.term.dispose();
        runtimesRef.current.delete(tabId);
      }
      ptyKill(tabId).catch(() => {});
      containerRefs.current.delete(tabId);

      setTabs((prev) => {
        const next = prev.filter((t) => t.id !== tabId);
        // If closing active tab, switch to adjacent
        setActiveTabId((currentActive) => {
          if (currentActive !== tabId) return currentActive;
          if (next.length === 0) return null;
          const oldIdx = prev.findIndex((t) => t.id === tabId);
          const newIdx = Math.min(oldIdx, next.length - 1);
          return next[newIdx].id;
        });
        return next;
      });
    },
    []
  );

  // ── Close all tabs (panel close) ─────────────────────

  const closeAll = useCallback(() => {
    for (const [tabId, rt] of runtimesRef.current) {
      rt.observer?.disconnect();
      rt.sse?.close();
      rt.term.dispose();
      ptyKill(tabId).catch(() => {});
    }
    runtimesRef.current.clear();
    containerRefs.current.clear();
    setTabs([]);
    setActiveTabId(null);
    onClose();
  }, [onClose]);

  // ── Rename handling ──────────────────────────────────

  const startRename = useCallback(
    (tabId: string) => {
      const tab = tabs.find((t) => t.id === tabId);
      if (tab) {
        setRenameId(tabId);
        setRenameValue(tab.label);
      }
    },
    [tabs]
  );

  const commitRename = useCallback(() => {
    if (renameId && renameValue.trim()) {
      setTabs((prev) =>
        prev.map((t) =>
          t.id === renameId ? { ...t, label: renameValue.trim() } : t
        )
      );
    }
    setRenameId(null);
    setRenameValue("");
  }, [renameId, renameValue]);

  // ── Auto-create first tab on mount ───────────────────

  const initializedRef = useRef(false);
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      createTab("shell");
    }
  }, [createTab]);

  // ── Initialize xterm for each tab ────────────────────

  useEffect(() => {
    for (const tab of tabs) {
      // Skip if already initialized
      if (runtimesRef.current.has(tab.id)) continue;

      const container = containerRefs.current.get(tab.id);
      if (!container) continue;

      const term = new Terminal({ ...TERM_OPTIONS, theme: getTerminalTheme() });
      const fitAddon = new FitAddon();
      const webLinksAddon = new WebLinksAddon();
      const searchAddon = new SearchAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(webLinksAddon);
      term.loadAddon(searchAddon);

      term.open(container);
      fitAddon.fit();

      // Write handler
      const ptyId = tab.id;
      term.onData((data: string) => {
        const encoded = btoa(
          Array.from(new TextEncoder().encode(data), (b) =>
            String.fromCharCode(b)
          ).join("")
        );
        ptyWrite(ptyId, encoded).catch(() => {});
      });

      // Resize handler
      term.onResize((dims: { cols: number; rows: number }) => {
        ptyResize(ptyId, dims.rows, dims.cols).catch(() => {});
      });

      const observer = new ResizeObserver(() => {
        fitAddon.fit();
      });
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

      // Spawn PTY
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
              for (let i = 0; i < raw.length; i++)
                bytes[i] = raw.charCodeAt(i);
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
          term.write(
            "\r\n\x1b[31mFailed to spawn terminal process.\x1b[0m\r\n"
          );
        });
    }
  }, [tabs, sessionId]);

  // ── Re-fit active tab when switching, expanding, or panel becomes visible ────

  useEffect(() => {
    if (!activeTabId || !visible) return;
    const rt = runtimesRef.current.get(activeTabId);
    if (rt) {
      // Small delay to let CSS changes apply (display:none → visible)
      requestAnimationFrame(() => rt.fit.fit());
    }
  }, [activeTabId, expanded, visible]);

  // ── Cleanup on unmount ───────────────────────────────

  useEffect(() => {
    return () => {
      for (const [tabId, rt] of runtimesRef.current) {
        rt.observer?.disconnect();
        rt.sse?.close();
        rt.term.dispose();
        ptyKill(tabId).catch(() => {});
      }
      runtimesRef.current.clear();
    };
  }, []);

  // ── Search functions ─────────────────────────────────

  const handleSearchChange = useCallback(
    (query: string) => {
      setSearchQuery(query);
      if (!activeTabId) return;
      const rt = runtimesRef.current.get(activeTabId);
      if (rt && query) {
        rt.search.findNext(query, { regex: false, caseSensitive: false });
      }
    },
    [activeTabId]
  );

  const searchNext = useCallback(() => {
    if (!activeTabId || !searchQuery) return;
    const rt = runtimesRef.current.get(activeTabId);
    rt?.search.findNext(searchQuery, { regex: false, caseSensitive: false });
  }, [activeTabId, searchQuery]);

  const searchPrev = useCallback(() => {
    if (!activeTabId || !searchQuery) return;
    const rt = runtimesRef.current.get(activeTabId);
    rt?.search.findPrevious(searchQuery, { regex: false, caseSensitive: false });
  }, [activeTabId, searchQuery]);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchQuery("");
    if (activeTabId) {
      const rt = runtimesRef.current.get(activeTabId);
      rt?.search.clearDecorations();
      // Re-focus terminal
      rt?.term.focus();
    }
  }, [activeTabId]);

  // Ctrl+F / Cmd+F to open search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        // Only handle if terminal panel is focused or search is open
        const termPanel = document.querySelector(".terminal-panel");
        if (!termPanel) return;
        // Check if focus is within the terminal panel
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

  // ── Render ───────────────────────────────────────────

  const activeTab = useMemo(
    () => tabs.find((t) => t.id === activeTabId) ?? null,
    [tabs, activeTabId]
  );

  return (
    <div className={`terminal-panel ${expanded ? "expanded" : ""}`}>
      {/* Tab bar + header actions */}
      <div className="terminal-panel-header">
        <div className="term-tab-bar">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              className={`term-tab ${tab.id === activeTabId ? "active" : ""}`}
              onClick={() => setActiveTabId(tab.id)}
              onDoubleClick={() => startRename(tab.id)}
              title={`${KIND_LABELS[tab.kind]} — ${tab.label}`}
            >
              <TermIcon size={11} className="term-tab-icon" />
              {renameId === tab.id ? (
                <input
                  className="term-tab-rename"
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  onBlur={commitRename}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitRename();
                    if (e.key === "Escape") {
                      setRenameId(null);
                      setRenameValue("");
                    }
                  }}
                  autoFocus
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <span className="term-tab-label">{tab.label}</span>
              )}
              {tabs.length > 1 && (
                <button
                  className="term-tab-close"
                  onClick={(e) => {
                    e.stopPropagation();
                    closeTab(tab.id);
                  }}
                  title="Close tab"
                >
                  <X size={10} />
                </button>
              )}
            </div>
          ))}

          {/* New tab button */}
          <div className="term-tab-new-wrapper">
            <button
              className="term-tab-new"
              onClick={() => {
                // Quick add shell; hold to see menu
                setKindMenuOpen((v) => !v);
              }}
              title="New terminal tab"
            >
              <Plus size={12} />
            </button>
            {kindMenuOpen && (
              <div className="term-kind-menu">
                {(["shell", "neovim", "git", "opencode"] as PtyKind[]).map(
                  (k) => (
                    <button
                      key={k}
                      className="term-kind-item"
                      onClick={() => createTab(k)}
                    >
                      {KIND_LABELS[k]}
                    </button>
                  )
                )}
              </div>
            )}
          </div>
        </div>

        <div className="terminal-panel-actions">
          {mcpAgentActive && (
            <span className="mcp-agent-indicator" title="AI agent active">
              <span className="mcp-agent-dot" />
            </span>
          )}
          <button
            onClick={() => {
              if (searchOpen) {
                closeSearch();
              } else {
                setSearchOpen(true);
                requestAnimationFrame(() => searchInputRef.current?.focus());
              }
            }}
            title="Find (Cmd+F)"
            aria-label="Search in terminal"
            className={searchOpen ? "active" : ""}
          >
            <Search size={14} />
          </button>
          <button
            onClick={() => setExpanded((v) => !v)}
            title="Toggle size"
            aria-label={expanded ? "Minimize terminal" : "Maximize terminal"}
          >
            {expanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
          </button>
          <button
            onClick={closeAll}
            title="Close terminal panel"
            aria-label="Close terminal panel"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Search bar */}
      {searchOpen && (
        <div className="term-search-bar">
          <Search size={12} className="term-search-icon" />
          <input
            ref={searchInputRef}
            className="term-search-input"
            type="text"
            placeholder="Find in terminal..."
            value={searchQuery}
            onChange={(e) => handleSearchChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                if (e.shiftKey) searchPrev();
                else searchNext();
              }
              if (e.key === "Escape") {
                e.preventDefault();
                closeSearch();
              }
            }}
          />
          <button className="term-search-nav" onClick={searchPrev} title="Previous match (Shift+Enter)">
            <ChevronUp size={14} />
          </button>
          <button className="term-search-nav" onClick={searchNext} title="Next match (Enter)">
            <ChevronDown size={14} />
          </button>
          <button className="term-search-close" onClick={closeSearch} title="Close search">
            <X size={12} />
          </button>
        </div>
      )}

      {/* Tab bodies (all mounted, only active visible) */}
      <div className="terminal-panel-body">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            ref={(el) => {
              if (el) containerRefs.current.set(tab.id, el);
            }}
            className="term-tab-body"
            style={{ display: tab.id === activeTabId ? "block" : "none" }}
          >
            {tab.status === "connecting" && (
              <div className="terminal-overlay">
                Spawning {KIND_LABELS[tab.kind]}...
              </div>
            )}
            {tab.status === "error" && (
              <div className="terminal-overlay error">
                Failed to start {KIND_LABELS[tab.kind]}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
