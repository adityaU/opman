import { useCallback, useMemo } from "react";
import { sendMessage, updateAutonomySettings } from "../api";
import type { ThemeColors, WorkspaceSnapshot, PersonalMemoryItem } from "../api";
import { applyThemeToCss } from "../utils/theme";

/* ── Input types ───────────────────────────────────────── */

export interface ChatCallbackInputs {
  activeSessionId: string | null;
  appState: any;
  selectedModel: any;
  personalMemory: PersonalMemoryItem[];
  activeProjectIndex: number;
  panels: {
    sidebar: { open: boolean };
    terminal: { open: boolean };
    editor: { open: boolean };
    git: { open: boolean };
  };
  setPanels: (p: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean }) => void;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  setSearchMatchIds: (ids: Set<string>) => void;
  setActiveSearchMatchId: (id: string | null) => void;
  setAutonomyMode: (mode: any) => void;
  setAssistantSignals: (fn: (prev: any[]) => any[]) => void;
  setActiveWorkspaceName: (name: string) => void;
  handleSelectSession: (id: string, projectIdx: number) => void;
}

/* ── Hook ──────────────────────────────────────────────── */

export function useChatCallbacks(inputs: ChatCallbackInputs) {
  const {
    activeSessionId, appState, selectedModel, personalMemory,
    activeProjectIndex, panels, setPanels, addToast,
    setSearchMatchIds, setActiveSearchMatchId,
    setAutonomyMode, setAssistantSignals, setActiveWorkspaceName,
    handleSelectSession,
  } = inputs;

  const handleThemeApplied = useCallback(
    (colors: ThemeColors) => { applyThemeToCss(colors); addToast("Theme applied", "success"); },
    [addToast],
  );

  const handleContextSubmit = useCallback(async (text: string) => {
    if (!activeSessionId) return;
    try {
      await sendMessage(activeSessionId, text, selectedModel ?? undefined);
      addToast("Context sent", "success");
    } catch { addToast("Failed to send context", "error"); }
  }, [activeSessionId, selectedModel, addToast]);

  const handleSearchMatchesChanged = useCallback(
    (matchIds: Set<string>, activeId: string | null) => {
      setSearchMatchIds(matchIds);
      setActiveSearchMatchId(activeId);
    }, [setSearchMatchIds, setActiveSearchMatchId],
  );

  const handlePanelError = useCallback(
    (msg: string) => addToast(msg, "error"),
    [addToast],
  );

  const handleRestoreWorkspace = useCallback((ws: WorkspaceSnapshot) => {
    setPanels(ws.panels);
    if (ws.session_id && ws.session_id !== activeSessionId) {
      handleSelectSession(ws.session_id, activeProjectIndex);
    }
    setActiveWorkspaceName(ws.name);
  }, [setPanels, activeSessionId, activeProjectIndex, handleSelectSession, setActiveWorkspaceName]);

  const buildCurrentSnapshot = useCallback((): WorkspaceSnapshot => ({
    name: "", created_at: new Date().toISOString(),
    panels: {
      sidebar: panels.sidebar.open,
      terminal: panels.terminal.open,
      editor: panels.editor.open,
      git: panels.git.open,
    },
    layout: { sidebar_width: 0, terminal_height: 0, side_panel_width: 0 },
    open_files: [], active_file: null, terminal_tabs: [],
    session_id: activeSessionId, git_branch: null, is_template: false,
  }), [panels, activeSessionId]);

  const onAutonomyChange = useCallback((mode: string) => {
    setAutonomyMode(mode as any);
    updateAutonomySettings(mode as any).catch(() => {});
  }, [setAutonomyMode]);

  const onDismissSignal = useCallback((id: string) => {
    setAssistantSignals((prev: any[]) => prev.filter((s: any) => s.id !== id));
  }, [setAssistantSignals]);

  const personalMemoryForInbox = useMemo(() =>
    personalMemory.filter((item: PersonalMemoryItem) => {
      if (!item?.scope) return false;
      if (item.scope === "global") return true;
      if (item.scope === "project") return item.project_index === activeProjectIndex;
      return item.session_id === activeSessionId;
    }), [personalMemory, activeProjectIndex, activeSessionId],
  );

  return {
    handleThemeApplied, handleContextSubmit, handleSearchMatchesChanged,
    handlePanelError, handleRestoreWorkspace, buildCurrentSnapshot,
    onAutonomyChange, onDismissSignal, personalMemoryForInbox,
  };
}
