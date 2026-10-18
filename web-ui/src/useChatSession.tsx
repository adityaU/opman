import { useCallback, useMemo, useEffect, useRef } from "react";
import type { ModalName } from "./hooks/useModalState";
import { useSidebarState } from "./hooks/useSidebarState";
import { useMobileState } from "./hooks/useMobileState";
import { useVirtualKeyboard } from "./hooks/useVirtualKeyboard";
import { useModelState } from "./hooks/useModelState";
import { useSessionEngine } from "./hooks/useSessionEngine";
import { useAssistantState } from "./hooks/useAssistantState";
import { useSessionRestore } from "./hooks/useSessionRestore";
import { useNotificationSignals } from "./hooks/useNotificationSignals";
import { useChatHandlers } from "./hooks/useChatHandlers";
import { useChatCallbacks } from "./hooks/useChatCallbacks";
import type { useChatBase } from "./useChatBase";

export function useChatSession(context: ReturnType<typeof useChatBase>) {
  const {
    runCommand, sse, appState, messages, permissions, questions,
    crossSessionPermissions, crossSessionQuestions, sessionStatus, watcherStatus, fileEditCount, refreshState,
    clearPermission, clearQuestion, addOptimisticMessage, clearOptimistic, newSessionMode, setUrlSession,
    selectProject, activeProjectIndex, activeProject, activeSessionId, activeSession, subSessionIds,
    modalState, toasts, addToast, removeToast, setRunnerChoice, runnerConfig,
    currentRunner, runnerSwitch, clearRunnerChoice, bindRunnerChoice, providers,
  } = context;
  // ── Bridge SSE toast events into the toast system ──
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as { message: string; level: string } | undefined;
      if (!detail?.message) return;
      const validLevels = ["success", "error", "info", "warning"] as const;
      const level = validLevels.includes(detail.level as typeof validLevels[number])
        ? (detail.level as typeof validLevels[number])
        : "info";
      addToast(detail.message, level, 4000);
    };
    window.addEventListener("opman:toast", handler);
    return () => window.removeEventListener("opman:toast", handler);
  }, [addToast]);

  // ── Persistent toasts for sub-session questions / permissions ──
  const crossToastMapRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    const map = crossToastMapRef.current;
    const currentIds = new Set<string>();

    // Only show toasts for sub-session permissions (children of active session)
    for (const perm of crossSessionPermissions) {
      if (!subSessionIds.has(perm.sessionID)) continue;
      currentIds.add(perm.id);
      if (!map.has(perm.id)) {
        const label = perm.toolName || "Permission";
        const tid = addToast(`**Permission request** from sub-session: *${label}*`, "warning", 0);
        map.set(perm.id, tid);
      }
    }
    // Only show toasts for sub-session questions
    for (const q of crossSessionQuestions) {
      if (!subSessionIds.has(q.sessionID)) continue;
      currentIds.add(q.id);
      if (!map.has(q.id)) {
        const label = q.title || "Question";
        const tid = addToast(`**Question** from sub-session: *${label}*`, "info", 0);
        map.set(q.id, tid);
      }
    }
    // Remove toasts for resolved items
    for (const [reqId, toastId] of map.entries()) {
      if (!currentIds.has(reqId)) {
        removeToast(toastId);
        map.delete(reqId);
      }
    }
  }, [crossSessionPermissions, crossSessionQuestions, subSessionIds, addToast, removeToast]);

  // ── Sidebar ──
  const sidebar = useSidebarState(true);

  // ── Mobile ──
  const mobile = useMobileState();
  useVirtualKeyboard();

  // ── Model / Agent ──
  const model = useModelState(messages, providers, activeSessionId);
  const handleRunnerChange = useCallback((runner: string) => {
    setRunnerChoice({ sessionId: activeSessionId, runner, isSwitch: activeSessionId !== null });
    // An explicit pick is also what the *next* session should open on. Restoring what this
    // runner was last set to is `useSessionEngine`'s job — it re-applies on every runner
    // change, so doing it here too would be a second answer to the same question.
    runnerConfig.rememberRunner(runner);
  }, [activeSessionId, runnerConfig]);

  // What the last assistant turn actually ran under. The default runner reports no
  // per-session configuration, so for its sessions this is the only record of one.
  const transcriptModel = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const info = messages[i]?.info;
      if (info?.role !== "assistant") continue;
      const modelID = info.modelID ?? (typeof info.model === "string" ? info.model : info.model?.modelID);
      const providerID = info.providerID ?? (typeof info.model === "object" ? info.model?.providerID : undefined);
      if (modelID && providerID) return { providerID, modelID };
    }
    return null;
  }, [messages]);

  // The composer's four controls, scoped to the session they describe. Every route into
  // them — engine palette, model picker modal, agent picker, slash command — goes through
  // these setters, so a change is recorded with the session's runner exactly once.
  const sessionEngine = useSessionEngine({
    activeSessionId, activeSession, currentRunner, runnerConfig, providers, transcriptModel,
    selectedModel: model.selectedModel, setSelectedModel: model.setSelectedModel,
    selectedAgent: model.selectedAgent, setSelectedAgent: model.setSelectedAgent,
  });
  const selectModel = sessionEngine.setModel;
  const selectAgent = sessionEngine.setAgent;
  const selectedModelId = model.currentModel || model.defaultModelDisplay;
  const selectedModelInfo = Object.values(providers.all)
    .flatMap((provider) => Object.values(provider.models))
    .find((providerModel) => providerModel.id === selectedModelId) as (typeof providers.all[number]["models"][string] & { variants?: Record<string, unknown> }) | undefined;
  const rawSupportedEfforts = selectedModelInfo?.reasoningEfforts || [];
  const variantEfforts = Object.entries(selectedModelInfo?.variants || {}).flatMap(([name, value]) => {
    if (!value || typeof value !== "object") return [name];
    const effort = (value as { reasoningEffort?: unknown }).reasoningEffort;
    return typeof effort === "string" ? [effort] : [name];
  });
  const supportedEfforts = [...rawSupportedEfforts, ...variantEfforts]
    .map((value: unknown) => {
      if (typeof value === "string") return value;
      if (!value || typeof value !== "object") return null;
      const entry = value as { reasoningEffort?: unknown; id?: unknown; value?: unknown };
      const label = entry.reasoningEffort ?? entry.id ?? entry.value;
      return typeof label === "string" ? label : null;
    })
    .filter((value): value is string => Boolean(value));
   const effortOptions = [...new Set(supportedEfforts)];

  // ── Restore the last session on a cold start ──
  useSessionRestore({
    appState,
    activeSessionId,
    projectIndex: activeProjectIndex,
    newSessionMode,
    selectSessionAt: setUrlSession,
  });

  // ── Assistant state ──
  const assistant = useAssistantState({
    activeSessionId, activeProject: activeProjectIndex,
    memoryOpen: modalState.modals.memory, autonomyOpen: modalState.modals.autonomy,
    routinesOpen: modalState.modals.routines,
  });

  // ── Notification signals ──
  useNotificationSignals({
    activeSessionId, sessionStatus,
    autonomyMode: assistant.autonomyMode, watcherStatus,
    permissions, questions,
    crossSessionPermissions, crossSessionQuestions,
    fileEditCount,
  });

  // ── Handlers (send, abort, command, session, etc.) ──
  const openModal = useCallback((name: string) => modalState.open(name as ModalName), [modalState]);
  // Ref-based getter so /copy reads current messages without invalidating handler memos.
  const messagesRef = useRef(messages);
  messagesRef.current = messages;
  const getMessages = useCallback(() => messagesRef.current, []);
  const handlers = useChatHandlers({
    activeSessionId, activeProjectIndex, appState,
    selectedModel: model.selectedModel, selectedAgent: model.selectedAgent,
    runnerForNewSession: currentRunner, runnerSwitch,
    selectedEffort: sessionEngine.effort, selectedPermission: sessionEngine.permission,
    setSending: model.setSending, setSelectedModel: selectModel,
    setSelectedAgent: selectAgent, clearRunnerChoice, bindRunnerChoice,
    setMobileInputHidden: mobile.setInputHidden,
    addToast, addOptimisticMessage, clearOptimistic, refreshState, refreshMessages: sse.refreshMessages,
    clearPermission, clearQuestion,
    closeMobileSidebarSilent: mobile.closeSidebarSilent,
    setUrlSession,
    selectProject,
    blockSessionAdoption: sse.blockSessionAdoption,
    runCommandId: runCommand,
    getMessages,
  });

  // ── Misc callbacks (theme, context, workspace, autonomy) ──
  const callbacks = useChatCallbacks({
    activeSessionId, appState,
    selectedModel: model.selectedModel,
    personalMemory: assistant.personalMemory,
    activeProjectIndex,
    addToast,
    setSearchMatchIds: modalState.setSearchMatchIds,
    setActiveSearchMatchId: modalState.setActiveSearchMatchId,
    setAutonomyMode: assistant.setAutonomyMode,
    handleSelectSession: handlers.handleSelectSession,
  });

  // ── Stable modal openers ──
  const closeModal = useCallback((name: string) => modalState.close(name as ModalName), [modalState]);
  const closeModalSilent = useCallback((name: string) => modalState.closeSilent(name as ModalName), [modalState]);
  const openAddProject = useCallback(() => modalState.open("addProject"), [modalState]);
  const openModelPicker = useCallback(() => modalState.open("modelPicker"), [modalState]);
  const openAgentPicker = useCallback(() => modalState.open("agentPicker"), [modalState]);

  return {
    ...context,
    crossToastMapRef, sidebar, mobile, model, handleRunnerChange, transcriptModel,
    sessionEngine, selectModel, selectAgent, selectedModelId, selectedModelInfo, rawSupportedEfforts,
    variantEfforts, supportedEfforts, effortOptions, assistant, openModal, messagesRef,
    getMessages, handlers, callbacks, closeModal, closeModalSilent, openAddProject,
    openModelPicker, openAgentPicker,
  };
}
