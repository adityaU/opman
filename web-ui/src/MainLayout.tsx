import React, { useState, useEffect, useCallback, useRef } from "react";
import {
  AppState,
  SessionStats,
  ThemeColors,
  fetchAppState,
  fetchSessionStats,
  fetchTheme,
  togglePanel,
  focusPanel,
  createEventsSSE,
} from "./api";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { XtermPanel } from "./XtermPanel";

/** Map ThemeColors fields to CSS custom properties on :root */
function applyThemeToCss(colors: ThemeColors) {
  const root = document.documentElement.style;
  root.setProperty("--color-primary", colors.primary);
  root.setProperty("--color-secondary", colors.secondary);
  root.setProperty("--color-accent", colors.accent);
  root.setProperty("--color-bg", colors.background);
  root.setProperty("--color-bg-panel", colors.background_panel);
  root.setProperty("--color-bg-element", colors.background_element);
  root.setProperty("--color-text", colors.text);
  root.setProperty("--color-text-muted", colors.text_muted);
  root.setProperty("--color-border", colors.border);
  root.setProperty("--color-border-active", colors.border_active);
  root.setProperty("--color-border-subtle", colors.border_subtle);
  root.setProperty("--color-error", colors.error);
  root.setProperty("--color-warning", colors.warning);
  root.setProperty("--color-success", colors.success);
  root.setProperty("--color-info", colors.info);
}

export function MainLayout() {
  const [state, setState] = useState<AppState | null>(null);
  const [stats, setStats] = useState<SessionStats | null>(null);
  const [busySessions, setBusySessions] = useState<Set<string>>(new Set());
  const pollRef = useRef<ReturnType<typeof setInterval>>();

  // Fetch app state
  const refreshState = useCallback(async () => {
    try {
      const s = await fetchAppState();
      setState(s);

      // Collect busy sessions
      const busy = new Set<string>();
      for (const p of s.projects) {
        for (const sid of p.busy_sessions) busy.add(sid);
      }
      setBusySessions(busy);

      // Fetch stats for active session
      const proj = s.projects[s.active_project];
      if (proj?.active_session) {
        const st = await fetchSessionStats(proj.active_session);
        setStats(st);
      }
    } catch (e) {
      console.error("Failed to fetch state:", e);
    }
  }, []);

  // Initial load + SSE event stream for live updates
  useEffect(() => {
    refreshState();

    // Fetch and apply the initial theme
    fetchTheme().then((colors) => {
      if (colors) applyThemeToCss(colors);
    });

    // Poll state periodically as fallback
    pollRef.current = setInterval(refreshState, 3000);

    // SSE for live events
    const sse = createEventsSSE();
    sse.addEventListener("state_changed", () => {
      refreshState();
    });
    sse.addEventListener("session_busy", (e: MessageEvent) => {
      setBusySessions((prev) => new Set([...prev, e.data]));
    });
    sse.addEventListener("session_idle", (e: MessageEvent) => {
      setBusySessions((prev) => {
        const next = new Set(prev);
        next.delete(e.data);
        return next;
      });
    });
    sse.addEventListener("stats_updated", (e: MessageEvent) => {
      try {
        setStats(JSON.parse(e.data));
      } catch {}
    });
    sse.addEventListener("theme_changed", (e: MessageEvent) => {
      try {
        const colors: ThemeColors = JSON.parse(e.data);
        applyThemeToCss(colors);
      } catch {}
    });

    return () => {
      clearInterval(pollRef.current);
      sse.close();
    };
  }, [refreshState]);

  const handleTogglePanel = useCallback(
    async (panel: string) => {
      await togglePanel(panel);
      refreshState();
    },
    [refreshState]
  );

  const handleFocusPanel = useCallback(
    async (panel: string) => {
      await focusPanel(panel);
      refreshState();
    },
    [refreshState]
  );

  // Mobile sidebar drawer state
  const [sidebarOpen, setSidebarOpen] = useState(false);

  if (!state) {
    return (
      <div className="panel-empty">
        Connecting to opman...
      </div>
    );
  }

  const activeProj = state.projects[state.active_project] || null;
  const panels = state.panels;

  return (
    <div className="app-layout">
      {/* Mobile header with hamburger + project name */}
      <div className="mobile-header">
        <button
          className="mobile-hamburger"
          onClick={() => setSidebarOpen((v) => !v)}
          aria-label="Toggle sidebar"
        >
          {sidebarOpen ? "\u2715" : "\u2630"}
        </button>
        <span className="mobile-project-name">
          {activeProj?.name || "opman"}
        </span>
      </div>

      {/* Overlay for mobile drawer */}
      <div
        className={`sidebar-overlay ${sidebarOpen ? "visible" : ""}`}
        onClick={() => setSidebarOpen(false)}
      />

      <div className="app-content">
        {/* Sidebar — always rendered, CSS handles mobile drawer visibility */}
        {panels.sidebar && (
          <Sidebar
            projects={state.projects}
            activeProject={state.active_project}
            busySessions={busySessions}
            onRefresh={refreshState}
            isDrawerOpen={sidebarOpen}
            onClose={() => setSidebarOpen(false)}
          />
        )}
        {/* Main panels area */}
        <div className="panels-container">
          {/* Top row: OpenCode info + Neovim + Git */}
          <div className="panels-top">
            {/* OpenCode pane — independent web PTY running `opencode attach` */}
            {panels.terminal_pane && (
              <div
                className={`panel ${state.focused === "TerminalPane" ? "focused" : "dimmed"}`}
                onClick={() => handleFocusPanel("TerminalPane")}
              >
                <div className="panel-header">
                  <span className="panel-title">
                    OpenCode
                    {activeProj?.active_session && (
                      <span style={{ fontSize: 10, color: "var(--text-dim)" }}>
                        {" "}
                        ({activeProj.active_session.slice(0, 8)})
                      </span>
                    )}
                  </span>
                </div>
                <div className="panel-body">
                  <XtermPanel
                    kind="opencode"
                    focused={state.focused === "TerminalPane"}
                    sessionId={activeProj?.active_session ?? undefined}
                  />
                </div>
              </div>
            )}

            {/* Neovim pane — independent web PTY */}
            {panels.neovim_pane && (
              <div
                className={`panel ${state.focused === "NeovimPane" ? "focused" : "dimmed"}`}
                onClick={() => handleFocusPanel("NeovimPane")}
              >
                <div className="panel-header">
                  <span className="panel-title">Neovim</span>
                </div>
                <div className="panel-body">
                  <XtermPanel
                    kind="neovim"
                    focused={state.focused === "NeovimPane"}
                  />
                </div>
              </div>
            )}

            {/* Git pane — independent web PTY */}
            {panels.git_panel && (
              <div
                className={`panel ${state.focused === "GitPanel" ? "focused" : "dimmed"}`}
                onClick={() => handleFocusPanel("GitPanel")}
              >
                <div className="panel-header">
                  <span className="panel-title">Git</span>
                </div>
                <div className="panel-body">
                  <XtermPanel
                    kind="git"
                    focused={state.focused === "GitPanel"}
                  />
                </div>
              </div>
            )}
          </div>

          {/* Bottom row: Integrated terminal — independent web PTY */}
          {panels.integrated_terminal && (
            <div className="panels-bottom" style={{ height: "33%" }}>
              <div
                className={`panel ${state.focused === "IntegratedTerminal" ? "focused" : "dimmed"}`}
                onClick={() => handleFocusPanel("IntegratedTerminal")}
                style={{ width: "100%" }}
              >
                <div className="panel-header">
                  <span className="panel-title">Terminal</span>
                </div>
                <div className="panel-body">
                  <XtermPanel
                    kind="shell"
                    focused={state.focused === "IntegratedTerminal"}
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Status bar */}
      <StatusBar
        project={activeProj}
        stats={stats}
        sessionStatus="idle"
        sidebarOpen={panels.sidebar}
        terminalOpen={panels.terminal_pane}
        neovimOpen={false}
        gitOpen={false}
        onToggleSidebar={() => handleTogglePanel("sidebar")}
        onToggleTerminal={() => handleTogglePanel("terminal_pane")}
        onToggleNeovim={() => {}}
        onToggleGit={() => {}}
        onOpenCommandPalette={() => {}}
      />
    </div>
  );
}
