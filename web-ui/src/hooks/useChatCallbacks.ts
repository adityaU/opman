import { useCallback } from "react";
import { sendMessage, updateAutonomySettings } from "../api";
import type { ThemeColors, PersonalMemoryItem } from "../api";
import { applyThemeToCss } from "../utils/theme";

/* ── Input types ───────────────────────────────────────── */

export interface ChatCallbackInputs {
  activeSessionId: string | null;
  appState: any;
  selectedModel: any;
  personalMemory: PersonalMemoryItem[];
  activeProjectIndex: number;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
  setSearchMatchIds: (ids: Set<string>) => void;
  setActiveSearchMatchId: (id: string | null) => void;
  setAutonomyMode: (mode: any) => void;
  handleSelectSession: (id: string, projectIdx: number) => void;
}

/* ── Hook ──────────────────────────────────────────────── */

export function useChatCallbacks(inputs: ChatCallbackInputs) {
  const {
    activeSessionId, selectedModel, addToast,
    setSearchMatchIds, setActiveSearchMatchId,
    setAutonomyMode,
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

  const onAutonomyChange = useCallback((mode: string) => {
    setAutonomyMode(mode as any);
    updateAutonomySettings(mode as any).catch(() => {});
  }, [setAutonomyMode]);

  return {
    handleThemeApplied, handleContextSubmit, handleSearchMatchesChanged,
    handlePanelError, onAutonomyChange,
  };
}
