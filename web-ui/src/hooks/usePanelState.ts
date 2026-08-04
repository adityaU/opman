import { useState, useCallback, useEffect, useMemo } from "react";
import { useResizable } from "./useResizable";

const PANEL_ORDER_KEY = "opman_panel_order";
const DEFAULT_PANEL_ORDER = ["editor", "git", "terminal", "debug"];

function loadPanelOrder(): string[] {
  try {
    const stored = JSON.parse(localStorage.getItem(PANEL_ORDER_KEY) || "null");
    if (!Array.isArray(stored)) return DEFAULT_PANEL_ORDER;
    const valid = stored.filter((id): id is string => DEFAULT_PANEL_ORDER.includes(id));
    return [...valid, ...DEFAULT_PANEL_ORDER.filter((id) => !valid.includes(id))];
  } catch {
    return DEFAULT_PANEL_ORDER;
  }
}

interface UsePanelStateOptions {
  initialPanels: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean; debug?: boolean };
  mcpEditorOpenPath: string | null;
  mcpTerminalFocusId: string | null;
  clearMcpEditorOpen: () => void;
  clearMcpTerminalFocus: () => void;
}

export function usePanelState({
  initialPanels,
  mcpEditorOpenPath,
  mcpTerminalFocusId,
  clearMcpEditorOpen,
  clearMcpTerminalFocus,
}: UsePanelStateOptions) {
  // ── Open state ──
  const [sidebarOpen, setSidebarOpen] = useState(initialPanels.sidebar);
  const [terminalOpen, setTerminalOpen] = useState(initialPanels.terminal);
  const [neovimOpen, setNeovimOpen] = useState(initialPanels.editor);
  const [gitOpen, setGitOpen] = useState(initialPanels.git);
  const [panelOrder, setPanelOrder] = useState(loadPanelOrder);

  useEffect(() => {
    try { localStorage.setItem(PANEL_ORDER_KEY, JSON.stringify(panelOrder)); } catch { /* storage unavailable */ }
  }, [panelOrder]);
  const [debugOpen, setDebugOpen] = useState(initialPanels.debug ?? false);

  // ── Mounted tracking (stay mounted once first opened) ──
  const [terminalMounted, setTerminalMounted] = useState(initialPanels.terminal);
  const [editorMounted, setEditorMounted] = useState(initialPanels.editor);
  const [gitMounted, setGitMounted] = useState(initialPanels.git);

  useEffect(() => { if (terminalOpen) setTerminalMounted(true); }, [terminalOpen]);
  useEffect(() => { if (neovimOpen) setEditorMounted(true); }, [neovimOpen]);
  useEffect(() => { if (gitOpen) setGitMounted(true); }, [gitOpen]);

  // ── MCP: auto-open editor when AI agent opens a file ──
  useEffect(() => {
    if (mcpEditorOpenPath) {
      setNeovimOpen(true);
      setEditorMounted(true);
      const timer = setTimeout(() => clearMcpEditorOpen(), 100);
      return () => clearTimeout(timer);
    }
  }, [mcpEditorOpenPath, clearMcpEditorOpen]);

  // ── MCP: auto-open/focus terminal when AI agent focuses a terminal ──
  useEffect(() => {
    if (mcpTerminalFocusId) {
      setTerminalOpen(true);
      setTerminalMounted(true);
      clearMcpTerminalFocus();
    }
  }, [mcpTerminalFocusId, clearMcpTerminalFocus]);

  // ── Resizable panels ──
  const sidebarResize = useResizable({ initialSize: 280, minSize: 200, maxSize: 500 });
  const sidePanelResize = useResizable({ initialSize: 500, minSize: 300, maxSize: 900, reverse: true });
  const terminalResize = useResizable({ initialSize: 250, minSize: 120, maxSize: 600, direction: "vertical", reverse: true });

  // ── Toggle & close callbacks ──
  const toggleSidebar = useCallback(() => setSidebarOpen((v) => !v), []);
  const toggleTerminal = useCallback(() => setTerminalOpen((v) => !v), []);
  const toggleNeovim = useCallback(() => setNeovimOpen((v) => !v), []);
  const toggleGit = useCallback(() => setGitOpen((v) => !v), []);
  const toggleDebug = useCallback(() => setDebugOpen((v) => !v), []);
  const reorderPanels = useCallback((source: string, target: string) => {
    setPanelOrder((current) => {
      if (source === target) return current;
      const next = current.filter((id) => id !== source);
      const index = next.indexOf(target);
      next.splice(index < 0 ? next.length : index, 0, source);
      return next;
    });
  }, []);

  const closeTerminal = useCallback(() => setTerminalOpen(false), []);
  const closeNeovim = useCallback(() => setNeovimOpen(false), []);
  const closeGit = useCallback(() => setGitOpen(false), []);
  const closeDebug = useCallback(() => setDebugOpen(false), []);

  // ── Focused panel ──
  const [focusedPanel, setFocusedPanel] = useState<"sidebar" | "chat" | "side">("chat");
  const focusSidebar = useCallback(() => setFocusedPanel("sidebar"), []);
  const focusChat = useCallback(() => setFocusedPanel("chat"), []);
  const focusSide = useCallback(() => setFocusedPanel("side"), []);

  const hasSidePanel = neovimOpen || gitOpen || debugOpen;

  return useMemo(() => ({
    sidebar: { open: sidebarOpen, setOpen: setSidebarOpen, toggle: toggleSidebar, resize: sidebarResize },
    terminal: {
      open: terminalOpen, setOpen: setTerminalOpen, mounted: terminalMounted,
      toggle: toggleTerminal, close: closeTerminal, resize: terminalResize,
    },
    editor: {
      open: neovimOpen, setOpen: setNeovimOpen, mounted: editorMounted,
      toggle: toggleNeovim, close: closeNeovim,
    },
    git: {
      open: gitOpen, setOpen: setGitOpen, mounted: gitMounted,
      toggle: toggleGit, close: closeGit,
    },
    debug: {
      open: debugOpen, setOpen: setDebugOpen,
      toggle: toggleDebug, close: closeDebug,
    },
    sidePanel: { hasPanel: hasSidePanel, resize: sidePanelResize },
    panelOrder,
    reorderPanels,
    focused: focusedPanel,
    focusSidebar,
    focusChat,
    focusSide,
  }), [
    sidebarOpen, toggleSidebar, sidebarResize,
    terminalOpen, terminalMounted, toggleTerminal, closeTerminal, terminalResize,
    neovimOpen, editorMounted, toggleNeovim, closeNeovim,
    gitOpen, gitMounted, toggleGit, closeGit,
    debugOpen, toggleDebug, closeDebug,
    hasSidePanel, sidePanelResize, panelOrder, reorderPanels,
    focusedPanel, focusSidebar, focusChat, focusSide,
  ]);
}

export type PanelState = ReturnType<typeof usePanelState>;
