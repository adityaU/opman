import React, { useState, useCallback, useMemo, useEffect, useRef } from "react";
import { useSSE } from "./hooks/useSSE";
import { useToast } from "./hooks/useToast";
import { useProviders } from "./hooks/useProviders";
import { useBookmarks } from "./hooks/useBookmarks";
import { useModalState } from "./hooks/useModalState";
import type { ModalName } from "./hooks/useModalState";
import { useSidebarState } from "./hooks/useSidebarState";
import { useMobileState } from "./hooks/useMobileState";
import { useVirtualKeyboard } from "./hooks/useVirtualKeyboard";
import { useModelState } from "./hooks/useModelState";
import { useRunnerConfig } from "./hooks/useRunnerConfig";
import { useSessionEngine } from "./hooks/useSessionEngine";
import { useAssistantState } from "./hooks/useAssistantState";
import { useSessionRestore } from "./hooks/useSessionRestore";
import { useSessionSelection } from "./hooks/useSessionSelection";
import { useNotificationSignals } from "./hooks/useNotificationSignals";
import { useChatHandlers } from "./hooks/useChatHandlers";
import { useChatCallbacks } from "./hooks/useChatCallbacks";
import { buildCommandHandlers } from "./chatLayoutCommands";
import { useCommands, useWhenContext } from "./keybindings/useCommand";
import { useKeymapContext } from "./keybindings/KeymapContext";
import { defaultRunner } from "./chatSessionHandlers";
import { ChatMainArea } from "./ChatMainArea";
import { ModalLayer } from "./ModalLayer";
import { MobileDock } from "./MobileDock";
import { ToastContainer } from "./ToastContainer";
import { getPersistedThemeMode, applyThemeMode } from "./theme-selector/persistence";
import type { ThemeMode } from "./theme-selector/persistence";
import { getPersistedAppearance, initAppearance } from "./utils/appearance";
import type { Appearance } from "./utils/appearance";
import { KanbanView } from "./kanban/KanbanView";
import { SettingsPage, useSettingsRoute } from "./settings-page";
import { useKanbanViewState } from "./kanban/useKanbanViewState";
import { useSessionTaskLinks } from "./sidebar/useSessionTaskLinks";
import { EditorOpenProvider } from "./tool-call/EditorOpenContext";
import type { FileOpenRequest } from "./code-editor/types";
import { StartupGate } from "./StartupGate";
import { isMobileViewport } from "./hooks/useIsMobile";
import { useWorkspaceShellProps } from "./workspace/useWorkspaceShellProps";

export function ChatLayout() {
  // Latches once the app has painted for the first time; see the startup gate
  // below.
  const hasStartedRef = useRef(false);

  // A `/name` in the composer resolves to a command id and runs through the same registry
  // as its chord, so the two can never drift into separate implementations.
  const { runCommand } = useKeymapContext();

  // ── Core SSE state ──
  const sse = useSSE();
  const {
    appState, messages, stats, busySessions, permissions, questions,
    crossSessionPermissions, crossSessionQuestions, sessionStatus,
    isLoadingMessages, isLoadingOlder, hasOlderMessages, totalMessageCount,
    watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpAgentActivity,
    refreshState, clearPermission, clearQuestion,
    clearMcpEditorOpen, clearMcpTerminalFocus,
    addOptimisticMessage, clearOptimistic, loadOlderMessages, beginSessionSwitch,
    isSessionBusy,
  } = sse;

  // Stable serialized key — changes only when the actual set of busy IDs changes.
  // Consumed by sidebar children that need to re-render on busy state change,
  // while avoiding passing the raw Set (which gets a new reference every event).
  const busyKey = useMemo(() => {
    if (busySessions.size === 0) return "";
    return Array.from(busySessions).sort().join(",");
  }, [busySessions]);

  // ── Active session + project (single source of truth) ──
  const {
    sessionId: urlSessionId,
    projectIndex: urlProjectIndex,
    newSessionMode,
    selectSessionAt: setUrlSession,
    selectProject,
  } = useSessionSelection({ appState, beginSessionSwitch });

  // ── Derived app state ──
  // URL is the single source of truth for active session + project.
  // Fall back to server state only when URL has no session yet (initial load).
  // URL is the sole source of truth for the active project — never fall back to server state.
  const activeProjectIndex = urlProjectIndex;
  const activeProject = appState ? appState.projects[activeProjectIndex] ?? null : null;
  const activeSessionId = newSessionMode ? null : (urlSessionId ?? activeProject?.active_session ?? null);
  const activeSession = activeProject?.sessions?.find((session: any) => session.id === activeSessionId);

  // Build the set of sub-session IDs (children of the active session)
  const subSessionIds = useMemo(() => {
    if (!activeSessionId || !activeProject?.sessions) return new Set<string>();
    const ids = new Set<string>();
    for (const s of activeProject.sessions) {
      if (s.parentID === activeSessionId) ids.add(s.id);
    }
    return ids;
  }, [activeSessionId, activeProject?.sessions]);

  // Only include cross-session permissions/questions from sub-sessions (not unrelated sessions)
  const allPermissions = useMemo(
    () => [...permissions, ...crossSessionPermissions.filter((p) => subSessionIds.has(p.sessionID))],
    [permissions, crossSessionPermissions, subSessionIds],
  );
  const allQuestions = useMemo(
    () => [...questions, ...crossSessionQuestions],
    [questions, crossSessionQuestions],
  );

  // ── Theme ──
  const [themeMode, setThemeMode] = useState<ThemeMode>(getPersistedThemeMode);
  const [appearance, setAppearanceState] = useState<Appearance>(getPersistedAppearance);
  useEffect(() => {
    applyThemeMode(themeMode);
    initAppearance();
    // Note: fetchThemePair() is already called in useSSE on mount —
    // no need to duplicate it here.
  }, []);

  // ── Modals / Toast / Providers / Bookmarks ──
  const modalState = useModalState({ onOpen: sse.blockSessionAdoption });
  const { toasts, addToast, removeToast } = useToast();
  // A runner pick belongs to the session it was made on: if it outlived that
  // session it would name a runner for the *next* one, which the backend reads
  // as a switch request and answers by forking a handoff session.
  // `isSwitch` records *when* the pick was made — choosing a runner before the
  // session exists configures it, while choosing one for a live session asks to
  // move that conversation. Only the second is a handoff request — and a session
  // whose runner label is briefly wrong (the label lands after creation) must
  // never turn the first kind into the second.
  const [runnerChoice, setRunnerChoice] = useState<
    { sessionId: string | null; runner: string; isSwitch: boolean } | null
  >(null);
  // What each runner was last configured with — model, agent, effort, permission
  // — kept per runner and across reloads, because none of those values mean
  // anything outside the runner they were chosen in.
  const runnerConfig = useRunnerConfig();
  const pickedRunner = runnerChoice && runnerChoice.sessionId === activeSessionId ? runnerChoice.runner : null;
  const availableRunners = useMemo<string[]>(
    () => appState?.runners || ["opencode", "claude-code", "claude", "codex"],
    [appState?.runners],
  );
  // A brand-new session has no runner of its own, and the server default is not
  // what the user was working in. Prefer their last pick — but only while it is
  // still on offer, so a renamed or removed ACP agent can never be sent.
  const rememberedRunner = runnerConfig.lastRunner();
  const fallbackRunner = availableRunners.includes(rememberedRunner)
    ? rememberedRunner
    : defaultRunner(appState);
  const currentRunner = pickedRunner || activeSession?.runner || fallbackRunner;
  const runnerSwitch = pickedRunner && runnerChoice?.isSwitch && pickedRunner !== activeSession?.runner
    ? pickedRunner
    : null;
  const clearRunnerChoice = useCallback(() => setRunnerChoice(null), []);
  /** Follow the pick onto the session it produced, without re-arming a switch. */
  const bindRunnerChoice = useCallback(
    (sessionId: string, runner: string) => setRunnerChoice({ sessionId, runner, isSwitch: false }),
    [],
  );
  const providers = useProviders(currentRunner);
  const { isBookmarked, toggleBookmark } = useBookmarks();

  // ── Settings page (its own route: /settings) ──
  const settings = useSettingsRoute();
  // Leaving settings returns to the conversation that was open, or the chat root.
  const leaveSettings = useCallback(() => {
    if (activeSessionId) setUrlSession(activeSessionId, activeProjectIndex);
    else selectProject(activeProjectIndex);
  }, [activeSessionId, activeProjectIndex, setUrlSession, selectProject]);

  // ── Kanban board view (its own route: /kanban) ──
  const {
    isKanbanView, boardProjectIndex, focusTaskId,
    openKanban, openKanbanTask, setBoardProject, clearFocusTask,
  } = useKanbanViewState();
  // Leaving the board returns to chat ("/") for the project the board was on —
  // honouring an in-board project switch. Prefer the session we came from when
  // it's the same project, else that project's backend-active session.
  const goToChat = useCallback(() => {
    const proj = boardProjectIndex ?? activeProjectIndex;
    const sid =
      (proj === activeProjectIndex ? activeSessionId : null) ??
      appState?.projects?.[proj]?.active_session ??
      null;
    if (sid) setUrlSession(sid, proj);
    else selectProject(proj);
  }, [boardProjectIndex, activeProjectIndex, activeSessionId, appState, setUrlSession, selectProject]);
  const toggleKanbanView = useCallback(() => {
    if (isKanbanView) goToChat();
    else openKanban(activeProjectIndex);
  }, [isKanbanView, goToChat, openKanban, activeProjectIndex]);
  // Sidebar back-link opens the task on the active project's board.
  const handleOpenKanbanTask = useCallback(
    (taskId: string) => openKanbanTask(taskId, activeProjectIndex),
    [openKanbanTask, activeProjectIndex],
  );

  // Reverse map (session → originating kanban task/lane) for the active project,
  // so the sidebar can tag kanban-launched sessions and link back to their task.
  const sessionTaskLinks = useSessionTaskLinks(activeProjectIndex, activeProject?.path);

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
  const effortOptions = supportedEfforts.length > 0 ? [...new Set(supportedEfforts)] : ["low", "medium", "high"];

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

  // ── Desktop workspace ──
  // What a pane that has never chosen an engine of its own sends on.
  const defaultEngine = useMemo(
    () => ({
      runner: currentRunner,
      model: model.selectedModel,
      agent: model.selectedAgent,
      effort: sessionEngine.effort,
      permission: sessionEngine.permission,
    }),
    [currentRunner, sessionEngine.effort, sessionEngine.permission, model.selectedAgent, model.selectedModel],
  );

  // Unfiltered, unlike `allPermissions`: that list is scoped to the active
  // session and its subagents, and a pane may be showing neither. Each pane
  // filters to its own session.
  const paneInteractions = useMemo(
    () => ({
      permissions: [...permissions, ...crossSessionPermissions],
      questions: [...questions, ...crossSessionQuestions],
    }),
    [crossSessionPermissions, crossSessionQuestions, permissions, questions],
  );

  const { workspaceProps, armTargeting, openKindHere, openFileInWorkspace } = useWorkspaceShellProps({
    appState,
    busySessions,
    defaultEngine,
    availableRunners,
    openModelPicker,
    openAgentPicker,
    runSlashCommand: (command: string, args?: string) => handlers.handleCommand(command, args),
    onError: (message: string) => addToast(message, "error"),
    subagentMessages,
    isBookmarked,
    toggleBookmark,
    openSession: (sessionId: string) => handlers.handleSelectSession(sessionId, activeProjectIndex),
    permissions: paneInteractions.permissions,
    questions: paneInteractions.questions,
    onPermissionReply: handlers.handlePermissionReply,
    onQuestionReply: handlers.handleQuestionReply,
    onQuestionDismiss: handlers.handleQuestionDismiss,
    searchOpen: modalState.modals.searchBar,
    closeSearch: () => modalState.close("searchBar"),
  });

  /**
   * Reveal a file, from a tool-card path click or from the MCP editor-open
   * event.
   *
   * On desktop the workspace answers: it reuses the files pane already on
   * screen, or splits one in beside the pane you clicked from. Everywhere else
   * — mobile, the board — there is one editor surface, and this is the request
   * it reads. `seq` rises on every ask so clicking the same path twice, having
   * browsed elsewhere in between, reveals it twice.
   */
  const [fileOpen, setFileOpen] = useState<FileOpenRequest | null>(null);
  const openFileInEditor = useCallback(
    (path: string, line?: number | null) => {
      if (openFileInWorkspace(path, line ?? null)) return;
      setFileOpen((previous) => ({ path, line: line ?? null, seq: (previous?.seq ?? 0) + 1 }));
      if (isMobileViewport() && mobile.activePanel !== "editor") mobile.togglePanel("editor");
    },
    [mobile, openFileInWorkspace],
  );

  // The MCP `web_editor` tool asking for a file is the same request arriving
  // over SSE, so it takes the same route. Cleared on arrival: it is an event,
  // and a latched one would re-fire on the next unrelated render.
  useEffect(() => {
    if (!mcpEditorOpenPath) return;
    openFileInEditor(mcpEditorOpenPath, mcpEditorOpenLine);
    clearMcpEditorOpen();
  }, [mcpEditorOpenPath, mcpEditorOpenLine, openFileInEditor, clearMcpEditorOpen]);

  // Likewise for `web_terminal`: the shell it names lives in the workspace now,
  // so all this can do is make sure a terminal is on screen.
  useEffect(() => {
    if (!mcpTerminalFocusId) return;
    openKindHere("terminal");
    clearMcpTerminalFocus();
  }, [mcpTerminalFocusId, openKindHere, clearMcpTerminalFocus]);

  /**
   * Hand a chat to a pane instead of navigating. Reports false when it could
   * not, so the caller falls back to the old single-transcript navigation:
   * mobile, the board, or a workspace that has not mounted.
   */
  const targetChat = useCallback(
    (sessionId: string | null, projectIdx: number, label: string) => {
      if (isMobileViewport() || isKanbanView) return false;
      const project = appState?.projects?.[projectIdx];
      if (!project) return false;
      return armTargeting({
        widget: { kind: "chat", projectPath: project.path, sessionId, engine: null },
        label,
      });
    },
    [appState, armTargeting, isKanbanView],
  );

  /**
   * Clicking a session in the sidebar.
   *
   * On desktop it arms the pane-target overlay instead of navigating: the
   * sidebar no longer commands one transcript, it hands a session to whichever
   * pane the user picks.
   */
  const handleSelectSessionOrTarget = useCallback(
    (sessionId: string, projectIdx: number) => {
      const project = appState?.projects?.[projectIdx];
      const session = project?.sessions?.find((candidate: any) => candidate.id === sessionId);
      if (targetChat(sessionId, projectIdx, session?.title || sessionId.slice(0, 8))) return;
      handlers.handleSelectSession(sessionId, projectIdx);
    },
    [appState, handlers, targetChat],
  );

  /**
   * Starting a new session — the same gesture, one step earlier. It has no id
   * to hand over yet, so the pane takes an unbound chat and creates the session
   * on first send; asking "which pane" only for existing sessions made the
   * button the one place in the workspace that seized a pane without asking.
   */
  const handleNewSessionOrTarget = useCallback(async () => {
    if (targetChat(null, activeProjectIndex, "New session")) return;
    await handlers.handleNewSession();
  }, [activeProjectIndex, handlers, targetChat]);
  const openMemoryActive = useCallback(() => modalState.openMemoryActive(), [modalState]);
  const openMemoryAll = useCallback(() => modalState.openMemoryAll(), [modalState]);
  const openCmdPalette = useCallback(() => modalState.open("commandPalette"), [modalState]);
  const closeSearchBar = useCallback(() => modalState.close("searchBar"), [modalState]);
  const openWatcher = useCallback(() => modalState.open("watcher"), [modalState]);
  const openCtxWindow = useCallback(() => modalState.open("contextWindow"), [modalState]);
  const onCompactCtx = useCallback(() => addToast("Compacting conversation...", "info"), [addToast]);

  // ── Keyboard: commands, not chords ──
  // The keymap owns which key runs what; this only says what the app can do.
  useCommands(buildCommandHandlers({
    openModal: modalState.open,
    toggleModal: modalState.toggle,
    closeTopModal: modalState.closeTopModal,
    toggleSidebar: sidebar.toggle,
    // The three panel chords open that widget in the focused pane. They no-op
    // where the workspace is not mounted — on mobile, whose dock owns those
    // surfaces, and on the board, which has no panes at all.
    toggleTerminal: () => { openKindHere("terminal"); },
    toggleEditor: () => { openKindHere("files"); },
    toggleGit: () => { openKindHere("git"); },
    toggleBoard: openKanban,
    newSession: handleNewSessionOrTarget,
    abortSession: handlers.handleAbort,
    copyTranscript: handlers.handleCopyTranscript,
    forwardToRunner: handlers.handleCommand,
    openMemoryActive,
    openMemoryAll,
    reloadApp: () => window.location.reload(),
  }));

  // Context keys read by `when` clauses. Every binding scoped to one of these
  // is inert until the matching surface says it applies. The editor, terminal
  // and git keys are published by the workspace instead: they describe the
  // focused pane, which only the workspace can see.
  useWhenContext({
    sessionActive: Boolean(activeSessionId),
    sessionBusy: model.sending,
    boardOpen: isKanbanView,
  });

  if (!appState) {
    return <StartupGate appState={null} connectionStatus={sse.connectionStatus} initialConnectionsReady={sse.initialConnectionsReady} activeSessionId={null} isLoadingMessages={false} providersLoading={providers.loading} />;
  }

  // The gate covers the *first* paint only. It used to be re-evaluated on every
  // render, so any later provider fetch — switching runners, a manual refresh —
  // tore the whole chat down to the loading screen and rebuilt it, losing all
  // transient UI state (open menus, in-progress input) along the way.
  const startupReady = appState.startup_ready !== false;
  const liveReady = sse.initialConnectionsReady;
  const workspaceReady = !activeSessionId || !isLoadingMessages;
  if (!hasStartedRef.current && (!startupReady || !liveReady || providers.loading || !workspaceReady)) {
    return <StartupGate appState={appState} connectionStatus={sse.connectionStatus} initialConnectionsReady={sse.initialConnectionsReady} activeSessionId={activeSessionId} isLoadingMessages={isLoadingMessages} providersLoading={providers.loading} />;
  }
  hasStartedRef.current = true;

  // Settings is a destination: it replaces the chat surface rather than floating over
  // it, so an unsaved skill or a login in flight cannot be dismissed by an Escape aimed
  // at something else. Toasts stay mounted; the mobile dock does not, because every
  // button on it targets a chat panel that is not on screen.
  if (settings.isSettingsView) {
    return (
      <div className="chat-layout">
        <SettingsPage
          section={settings.section}
          onSelectSection={settings.openSection}
          onExit={leaveSettings}
          appearance={appearance}
          onAppearanceChange={setAppearanceState}
          themeMode={themeMode}
          onThemeModeChange={setThemeMode}
          onThemeApplied={callbacks.handleThemeApplied}
          onError={(message) => addToast(message, "error")}
          runners={availableRunners}
        />
        <ToastContainer toasts={toasts} onDismiss={removeToast} />
      </div>
    );
  }

  return (
    <EditorOpenProvider value={openFileInEditor}>
    <div className="chat-layout">
      {mobile.sidebarOpen && <div className="sidebar-overlay visible" onClick={mobile.closeSidebar} />}
      <ChatMainArea
        isKanbanView={isKanbanView} onToggleKanban={toggleKanbanView}
        appState={appState} activeProject={activeProject} activeProjectIndex={activeProjectIndex}
        activeSessionId={activeSessionId}
        sessionStatus={sessionStatus} connectionStatus={sse.connectionStatus}
        stats={stats}
        messages={messages} isSessionBusy={isSessionBusy} busyKey={busyKey}
        isLoadingMessages={isLoadingMessages} isLoadingOlder={isLoadingOlder}
        hasOlderMessages={hasOlderMessages} totalMessageCount={totalMessageCount}
        subagentMessages={subagentMessages} defaultModelDisplay={model.defaultModelDisplay}
        selectedModel={model.selectedModel} selectedAgent={model.selectedAgent}
        handleModelSelected={handlers.handleModelSelected}
        selectedRunner={currentRunner} availableRunners={availableRunners}
        supportedEfforts={effortOptions} effort={sessionEngine.effort} permission={sessionEngine.permission}
        sending={model.sending} currentModel={model.currentModel}
        allPermissions={allPermissions} allQuestions={allQuestions}
        activeMemoryItems={assistant.activeMemoryItems}
        watcherStatus={watcherStatus} presenceClients={sse.presenceClients}
        contextLimit={model.currentModelContextLimit}
        onOpenWatcher={openWatcher} onOpenContextWindow={openCtxWindow} onToggleSidebar={sidebar.toggle}
        sidebarOpen={sidebar.open} focusedPanel={sidebar.focused}
        sidebarResize={sidebar.resize} searchBarOpen={modalState.modals.searchBar}
        searchMatchIds={modalState.searchMatchIds} activeSearchMatchId={modalState.activeSearchMatchId}
        mobileSidebarOpen={mobile.sidebarOpen} mobileInputHidden={mobile.inputHidden}
        isBookmarked={isBookmarked} toggleBookmark={toggleBookmark}
        handleSend={handlers.handleSend} handleAbort={handlers.handleAbort}
        handleCommand={handlers.handleCommand} handlePermissionReply={handlers.handlePermissionReply}
        handleQuestionReply={handlers.handleQuestionReply}
        handleQuestionDismiss={handlers.handleQuestionDismiss}
        workspace={workspaceProps}
        handleSelectSession={handleSelectSessionOrTarget} handleNewSession={handleNewSessionOrTarget}
        handleSwitchProject={handlers.handleSwitchProject} handleAgentChange={handlers.handleAgentChange}
        handleRunnerChange={handleRunnerChange}
        handleEffortChange={sessionEngine.setEffort}
        handlePermissionChange={sessionEngine.setPermission}
        handleSearchMatchesChanged={callbacks.handleSearchMatchesChanged}
        handleScrollDirection={mobile.handleScrollDirection}
        handlePromptContentChange={mobile.handlePromptContentChange}
        loadOlderMessages={loadOlderMessages}
        openAddProject={openAddProject} openModelPicker={openModelPicker}
        openAgentPicker={openAgentPicker} openMemory={openMemoryActive}
        openCommandPalette={openCmdPalette} closeSearchBar={closeSearchBar}
        closeMobileSidebar={mobile.closeSidebar} toggleMobileSidebar={mobile.toggleSidebar}
        focusSidebar={sidebar.focusSidebar} focusChat={sidebar.focusChat}
        handlePanelError={callbacks.handlePanelError}
        sessionTaskLinks={sessionTaskLinks} onOpenKanbanTask={handleOpenKanbanTask}
        focusTaskId={focusTaskId} clearFocusTask={clearFocusTask}
        boardProjectIndex={boardProjectIndex} onSelectBoardProject={setBoardProject}
      />
      <ModalLayer
        modals={modalState.modals} openModal={openModal} closeModal={closeModal} closeModalSilent={closeModalSilent}
        appState={appState} activeSessionId={activeSessionId} activeProject={activeProject}
        currentRunner={currentRunner}
        activeProjectIndex={activeProjectIndex}
        onCommand={handlers.handleCommand} onNewSession={handleNewSessionOrTarget}
        onSelectSession={handlers.handleSelectSession} onSend={handlers.handleSend}
        onModelSelected={handlers.handleModelSelected} onAgentChange={handlers.handleAgentChange}
        onContextSubmit={callbacks.handleContextSubmit}
        onCompactContext={onCompactCtx} onAutonomyChange={callbacks.onAutonomyChange}
        toggleSidebar={sidebar.toggle} toggleTerminal={() => openKindHere("terminal")}
        selectedModel={model.selectedModel} selectedAgent={model.selectedAgent}
        fileEditCount={fileEditCount}
        allPermissions={allPermissions} allQuestions={allQuestions}
        watcherStatus={watcherStatus}
        autonomyMode={assistant.autonomyMode}
        routineCache={assistant.routineCache}
        activeMemoryItems={assistant.activeMemoryItems}
        memoryFilterActive={modalState.memoryFilterActive}
        openMemoryAll={openMemoryAll}
        clearPermission={clearPermission} clearQuestion={clearQuestion}
      />
      <ToastContainer toasts={toasts} onDismiss={removeToast} />
      <MobileDock
        activePanel={mobile.activePanel} panelsMounted={mobile.panelsMounted}
        togglePanel={mobile.togglePanel} inputHidden={mobile.inputHidden}
        handleComposeButtonTap={mobile.handleComposeButtonTap}
        dockCollapsed={mobile.dockCollapsed} expandDock={mobile.expandDock}
        onOpenCommandPalette={openCmdPalette}
        activeSessionId={activeSessionId} activeProject={activeProject}
        fileOpen={fileOpen} mcpAgentActivity={mcpAgentActivity}
        onError={callbacks.handlePanelError} onSendToAI={handlers.handleSend}
        isKanbanView={isKanbanView} onToggleKanban={toggleKanbanView}
      />
    </div>
    </EditorOpenProvider>
  );
}
