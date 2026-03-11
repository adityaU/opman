import { useState, useCallback, useEffect } from "react";
import { useResizable } from "./useResizable";

interface UsePanelStateOptions {
  initialPanels: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean };
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

  const closeTerminal = useCallback(() => setTerminalOpen(false), []);
  const closeNeovim = useCallback(() => setNeovimOpen(false), []);
  const closeGit = useCallback(() => setGitOpen(false), []);

  // ── Focused panel ──
  const [focusedPanel, setFocusedPanel] = useState<"sidebar" | "chat" | "side">("chat");
  const focusSidebar = useCallback(() => setFocusedPanel("sidebar"), []);
  const focusChat = useCallback(() => setFocusedPanel("chat"), []);
  const focusSide = useCallback(() => setFocusedPanel("side"), []);

  return {
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
    sidePanel: { hasPanel: neovimOpen || gitOpen, resize: sidePanelResize },
    focused: focusedPanel,
    focusSidebar,
    focusChat,
    focusSide,
  };
}

export type PanelState = ReturnType<typeof usePanelState>;
