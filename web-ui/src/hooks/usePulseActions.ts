import { useCallback } from "react";
import type { ModalName } from "./useModalState";
import type { AutonomyMode, WorkspaceSnapshot } from "../api";
import {
  createRoutine,
  updateAutonomySettings,
  saveWorkspace,
  fetchWorkspaces,
} from "../api";

export interface UsePulseActionsOptions {
  assistantPulse: { action: string } | null;
  activeSessionId: string | null;
  activeProject: any;
  openModal: (name: ModalName) => void;
  setAutonomyMode: (mode: AutonomyMode) => void;
  setRoutineCache: React.Dispatch<React.SetStateAction<any[]>>;
  setWorkspaceCache: React.Dispatch<React.SetStateAction<WorkspaceSnapshot[]>>;
  addToast: (msg: string, type: "success" | "error" | "info" | "warning") => void;
}

/**
 * Creates the handler for running the top assistant-pulse recommendation action.
 */
export function usePulseActions(opts: UsePulseActionsOptions) {
  const {
    assistantPulse,
    activeSessionId,
    activeProject,
    openModal,
    setAutonomyMode,
    setRoutineCache,
    setWorkspaceCache,
    addToast,
  } = opts;

  const handleRunAssistantPulse = useCallback(() => {
    if (!assistantPulse) return;
    switch (assistantPulse.action) {
      case "open_inbox": openModal("inbox"); break;
      case "open_missions": openModal("missions"); break;
      case "open_memory": openModal("memory"); break;
      case "open_routines": openModal("routines"); break;
      case "open_delegation": openModal("delegation"); break;
      case "open_workspaces": openModal("workspaceManager"); break;
      case "open_autonomy": openModal("autonomy"); break;
      case "setup_daily_summary":
        createRoutine({
          name: "Daily Briefing",
          trigger: "daily_summary",
          action: "open_inbox",
          mission_id: null,
          session_id: null,
        })
          .then((routine) => {
            setRoutineCache((prev: any[]) => [routine, ...prev]);
            addToast("Daily briefing enabled", "success");
          })
          .catch(() => addToast("Failed to enable daily briefing", "error"));
        break;
      case "upgrade_autonomy_nudge":
        setAutonomyMode("nudge");
        updateAutonomySettings("nudge")
          .then(() => addToast("Autonomy set to Nudge", "success"))
          .catch(() => addToast("Failed to update autonomy", "error"));
        break;
      case "setup_daily_copilot":
        Promise.allSettled([
          createRoutine({
            name: "Daily Briefing",
            trigger: "daily_summary",
            action: "open_inbox",
            mission_id: null,
            session_id: null,
          }).then((routine) => {
            setRoutineCache((prev: any[]) => {
              if (prev.some((item: any) => item.name === routine.name)) return prev;
              return [routine, ...prev];
            });
          }),
          updateAutonomySettings("nudge").then(() => {
            setAutonomyMode("nudge");
          }),
          saveWorkspace({
            name: "Morning Review",
            created_at: new Date().toISOString(),
            panels: { sidebar: true, terminal: false, editor: false, git: true },
            layout: { sidebar_width: 320, terminal_height: 0, side_panel_width: 480 },
            open_files: [],
            active_file: null,
            terminal_tabs: [],
            session_id: activeSessionId,
            git_branch: activeProject?.git_branch ?? null,
            is_template: false,
            recipe_description: "Start the day with missions, inbox, and git context ready.",
            recipe_next_action: "Review the assistant summary, clear blockers, then choose the next mission.",
            is_recipe: true,
          }).then(() => {
            fetchWorkspaces().then((resp) => setWorkspaceCache(resp.workspaces)).catch(() => {});
          }),
        ]).finally(() => {
          addToast("Daily Copilot preset enabled", "success");
        });
        break;
    }
  }, [assistantPulse, activeSessionId, activeProject, openModal, setAutonomyMode, setRoutineCache, setWorkspaceCache, addToast]);

  return { handleRunAssistantPulse };
}
