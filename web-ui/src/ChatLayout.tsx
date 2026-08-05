import React, { useState, useCallback, useMemo, useEffect, useRef } from "react";
import { useSSE } from "./hooks/useSSE";
import { useKeyboard } from "./hooks/useKeyboard";
import { useToast } from "./hooks/useToast";
import { useProviders } from "./hooks/useProviders";
import { useBookmarks } from "./hooks/useBookmarks";
import { useModalState } from "./hooks/useModalState";
import type { ModalName } from "./hooks/useModalState";
import { usePanelState } from "./hooks/usePanelState";
import { useMobileState } from "./hooks/useMobileState";
import { useVirtualKeyboard } from "./hooks/useVirtualKeyboard";
import { useModelState } from "./hooks/useModelState";
import { useAssistantState } from "./hooks/useAssistantState";
import { useUrlRestore } from "./hooks/useUrlRestore";
import { useUrlSessionState } from "./hooks/useUrlSessionState";
import { useNotificationSignals } from "./hooks/useNotificationSignals";
import { usePulseActions } from "./hooks/usePulseActions";
import { useChatHandlers } from "./hooks/useChatHandlers";
import { useChatCallbacks } from "./hooks/useChatCallbacks";
import { buildKeyboardShortcuts } from "./chatLayoutKeyboard";
import { ChatMainArea } from "./ChatMainArea";
import { ModalLayer } from "./ModalLayer";
import { MobileDock } from "./MobileDock";
import { ToastContainer } from "./ToastContainer";
import { getPersistedThemeMode, applyThemeMode } from "./ThemeSelectorModal";
import type { ThemeMode } from "./ThemeSelectorModal";
import { getPersistedAppearance, initAppearance } from "./utils/appearance";
import type { Appearance } from "./utils/appearance";
import { SkillsUploadModal } from "./SkillsUploadModal";
import { KanbanView } from "./kanban/KanbanView";
import { useKanbanViewState } from "./kanban/useKanbanViewState";
import { useSessionTaskLinks } from "./sidebar/useSessionTaskLinks";
import { appNavigate } from "./utils/navigation";
import { EditorOpenProvider } from "./tool-call/EditorOpenContext";
import { StartupGate } from "./StartupGate";

function defaultPermissionForRunner(runner: string): string {
  if (runner === "claude" || runner === "claude-code") return "default";
  if (runner === "codex") return "on-request";
  return "default";
}

export function ChatLayout() {
  // ── Core SSE state ──
  const sse = useSSE();
  const {
    appState, messages, stats, busySessions, permissions, questions,
    crossSessionPermissions, crossSessionQuestions, sessionStatus,
    isLoadingMessages, isLoadingOlder, hasOlderMessages, totalMessageCount,
    watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpAgentActivity,
    refreshState, clearPermission, clearQuestion,
    clearMcpEditorOpen, openMcpEditor, clearMcpTerminalFocus,
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

  // ── URL-driven session state (single source of truth) ──
  const { urlSessionId, urlProjectIndex, newSessionMode, setUrlSession } = useUrlSessionState({
    appState, beginSessionSwitch,
  });

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
  const [selectedRunner, setSelectedRunner] = useState<string | null>(null);
  const [runnerSettings, setRunnerSettings] = useState<Record<string, { effort: string | null; permission: string }>>({});
  const currentRunner = selectedRunner || activeSession?.runner || (appState?.backend === "claude-code" ? "claude-code" : appState?.backend) || "opencode";
  const providers = useProviders(currentRunner);
  const currentSettings = runnerSettings[currentRunner] || {
    effort: null,
    permission: defaultPermissionForRunner(currentRunner),
  };
  const setRunnerSetting = useCallback((patch: Partial<{ effort: string | null; permission: string }>) => {
    setRunnerSettings((current) => ({
      ...current,
      [currentRunner]: {
        ...(current[currentRunner] || { effort: null, permission: defaultPermissionForRunner(currentRunner) }),
        ...patch,
      },
    }));
  }, [currentRunner]);
  const { isBookmarked, toggleBookmark } = useBookmarks();

  const [skillsUploadOpen, setSkillsUploadOpen] = useState(false);
  // Bumped each time the input's "Attach terminal" button is clicked, so the terminal
  // panel opens a fresh `claude attach` tab for the active session.
  const [terminalAttachNonce, setTerminalAttachNonce] = useState(0);

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
    else appNavigate(`/?project=${proj}`);
  }, [boardProjectIndex, activeProjectIndex, activeSessionId, appState, setUrlSession]);
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

  // ── Panels ──
  const panels = usePanelState({
    initialPanels: { sidebar: true, terminal: false, editor: false, git: false },
    mcpEditorOpenPath, mcpTerminalFocusId,
    clearMcpEditorOpen, clearMcpTerminalFocus,
  });

  const setPanels = useCallback((p: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean }) => {
    panels.sidebar.setOpen(p.sidebar);
    panels.terminal.setOpen(p.terminal);
    panels.editor.setOpen(p.editor);
    panels.git.setOpen(p.git);
  }, [panels]);

  // ── Mobile ──
  const mobile = useMobileState();
  useVirtualKeyboard();

  // ── Open a file in the editor from a tool-card path click ──
  // Desktop: usePanelState auto-opens the editor when mcpEditorOpenPath is set.
  // Mobile: the dock must be switched to the editor sheet explicitly.
  const openFileInEditor = useCallback((path: string, line?: number | null) => {
    openMcpEditor(path, line);
    if (typeof window !== "undefined" && window.innerWidth < 768 && mobile.activePanel !== "editor") {
      mobile.togglePanel("editor");
    }
  }, [openMcpEditor, mobile]);

  // ── Model / Agent ──
  const model = useModelState(messages, providers, activeSessionId);
  const handleRunnerChange = useCallback((runner: string) => {
    setSelectedRunner(runner);
    model.setSelectedAgent("");
  }, [model.setSelectedAgent]);
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

  // ── URL restore/sync ──
  useUrlRestore({
    appState, activeSessionId, activeProjectIndex, newSessionMode,
    panels: {
      sidebarOpen: panels.sidebar.open, terminalOpen: panels.terminal.open,
      neovimOpen: panels.editor.open, gitOpen: panels.git.open,
    },
    setPanels, setUrlSession,
  });

  // ── Assistant state ──
  const assistant = useAssistantState(
    {
      appState, activeSessionId, activeProject: activeProjectIndex,
      sessionStatus, permissions: allPermissions, questions: allQuestions,
      liveActivityEvents: sse.liveActivityEvents, watcherStatus,
      memoryOpen: modalState.modals.memory, autonomyOpen: modalState.modals.autonomy,
      routinesOpen: modalState.modals.routines, missionsOpen: modalState.modals.missions,
      delegationOpen: modalState.modals.delegation,
      workspaceManagerOpen: modalState.modals.workspaceManager,
      assistantCenterOpen: modalState.modals.assistantCenter,
    },
    {
      onOpenAssistantCenter: () => modalState.open("assistantCenter"),
    },
  );

  // ── Notification signals ──
  useNotificationSignals({
    activeSessionId, sessionStatus,
    autonomyMode: assistant.autonomyMode, watcherStatus,
    permissions, questions,
    crossSessionPermissions, crossSessionQuestions,
    fileEditCount,
    setAssistantSignals: assistant.setAssistantSignals,
  });

  // ── Pulse actions ──
  const { handleRunAssistantPulse } = usePulseActions({
    assistantPulse: assistant.assistantPulse,
    activeSessionId, activeProject,
    openModal: modalState.open,
    setAutonomyMode: assistant.setAutonomyMode,
    setRoutineCache: assistant.setRoutineCache,
    setWorkspaceCache: assistant.setWorkspaceCache,
    addToast,
  });

  // ── Handlers (send, abort, command, session, etc.) ──
  const openModal = useCallback((name: string) => modalState.open(name as ModalName), [modalState]);
  // Ref-based getter so /copy reads current messages without invalidating handler memos.
  const messagesRef = useRef(messages);
  messagesRef.current = messages;
  const getMessages = useCallback(() => messagesRef.current, []);
  const handlers = useChatHandlers({
    activeSessionId, activeProjectIndex, appState,
    selectedModel: model.selectedModel, selectedAgent: model.selectedAgent, selectedRunner,
    selectedEffort: currentSettings.effort, selectedPermission: currentSettings.permission,
    sending: model.sending, activeMemoryItems: assistant.activeMemoryItems,
    setSending: model.setSending, setSelectedModel: model.setSelectedModel,
    setSelectedAgent: model.setSelectedAgent, setSelectedRunner,
    setMobileInputHidden: mobile.setInputHidden,
    addToast, addOptimisticMessage, clearOptimistic, refreshState, refreshMessages: sse.refreshMessages,
    clearPermission, clearQuestion,
    setMobileSidebarOpen: mobile.setSidebarOpen,
    closeMobileSidebarSilent: mobile.closeSidebarSilent,
    setUrlSession,
    openModal,
    expectSessionSwitch: sse.expectSessionSwitch,
    blockSessionAdoption: sse.blockSessionAdoption,
    openMemoryAll: modalState.openMemoryAll,
    toggleSidebar: panels.sidebar.toggle, toggleTerminal: panels.terminal.toggle,
    toggleNeovim: panels.editor.toggle, toggleGit: panels.git.toggle,
    toggleDebug: panels.debug.toggle,
    toggleSplitView: () => modalState.toggle("splitView"),
    getMessages,
  });

  // ── Misc callbacks (theme, context, workspace, autonomy) ──
  const callbacks = useChatCallbacks({
    activeSessionId, appState,
    selectedModel: model.selectedModel,
    personalMemory: assistant.personalMemory,
    activeProjectIndex,
    panels: { sidebar: panels.sidebar, terminal: panels.terminal, editor: panels.editor, git: panels.git },
    setPanels, addToast,
    setSearchMatchIds: modalState.setSearchMatchIds,
    setActiveSearchMatchId: modalState.setActiveSearchMatchId,
    setAutonomyMode: assistant.setAutonomyMode,
    setAssistantSignals: assistant.setAssistantSignals,
    setActiveWorkspaceName: assistant.setActiveWorkspaceName,
    handleSelectSession: handlers.handleSelectSession,
  });

  // ── Stable modal openers ──
  const closeModal = useCallback((name: string) => modalState.close(name as ModalName), [modalState]);
  const closeModalSilent = useCallback((name: string) => modalState.closeSilent(name as ModalName), [modalState]);
  const openAddProject = useCallback(() => modalState.open("addProject"), [modalState]);
  const openModelPicker = useCallback(() => modalState.open("modelPicker"), [modalState]);
  const openAgentPicker = useCallback(() => modalState.open("agentPicker"), [modalState]);
  const openMemoryActive = useCallback(() => modalState.openMemoryActive(), [modalState]);
  const openMemoryAll = useCallback(() => modalState.openMemoryAll(), [modalState]);
  const openCmdPalette = useCallback(() => modalState.open("commandPalette"), [modalState]);
  const closeSearchBar = useCallback(() => modalState.close("searchBar"), [modalState]);
  const openWatcher = useCallback(() => modalState.open("watcher"), [modalState]);
  const openCtxWindow = useCallback(() => modalState.open("contextWindow"), [modalState]);
  const openAssistantCenter = useCallback(() => modalState.open("assistantCenter"), [modalState]);
  const onCompactCtx = useCallback(() => addToast("Compacting conversation...", "info"), [addToast]);
  const toggleSplitView = useCallback(() => modalState.toggle("splitView"), [modalState]);

  // ── Keyboard shortcuts ──
  useKeyboard(buildKeyboardShortcuts({
    openModal, closeTopModal: modalState.closeTopModal,
    toggleSidebar: panels.sidebar.toggle, toggleTerminal: panels.terminal.toggle,
    toggleNeovim: panels.editor.toggle, toggleGit: panels.git.toggle,
    handleNewSession: handlers.handleNewSession, toggleSplitView,
  }));

  if (!appState) {
    return <StartupGate appState={null} connectionStatus={sse.connectionStatus} initialConnectionsReady={sse.initialConnectionsReady} activeSessionId={null} isLoadingMessages={false} providersLoading={providers.loading} />;
  }

  const startupReady = appState.startup_ready !== false;
  const liveReady = sse.initialConnectionsReady;
  const workspaceReady = !activeSessionId || !isLoadingMessages;
  if (!startupReady || !liveReady || providers.loading || !workspaceReady) {
    return <StartupGate appState={appState} connectionStatus={sse.connectionStatus} initialConnectionsReady={sse.initialConnectionsReady} activeSessionId={activeSessionId} isLoadingMessages={isLoadingMessages} providersLoading={providers.loading} />;
  }

  return (
    <EditorOpenProvider value={openFileInEditor}>
    <div className="chat-layout">
      {mobile.sidebarOpen && <div className="sidebar-overlay visible" onClick={mobile.closeSidebar} />}
      <ChatMainArea
        terminalAttachNonce={terminalAttachNonce}
        onAttachTerminal={() => { panels.terminal.setOpen(true); setTerminalAttachNonce((n) => n + 1); }}
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
        selectedRunner={currentRunner} availableRunners={appState?.runners || ["opencode", "claude-code", "claude", "codex"]}
        supportedEfforts={effortOptions} effort={currentSettings.effort} permission={currentSettings.permission}
        sending={model.sending} currentModel={model.currentModel}
        allPermissions={allPermissions} allQuestions={allQuestions}
        activeMemoryItems={assistant.activeMemoryItems}
        mcpEditorOpenPath={mcpEditorOpenPath} mcpEditorOpenLine={mcpEditorOpenLine}
         watcherStatus={watcherStatus} presenceClients={sse.presenceClients} activeWorkspaceName={assistant.activeWorkspaceName}
         autonomyMode={assistant.autonomyMode} assistantPulse={assistant.assistantPulse} contextLimit={model.currentModelContextLimit}
         backend={appState?.backend} onRunAssistantPulse={handleRunAssistantPulse}
         onOpenWatcher={openWatcher} onOpenContextWindow={openCtxWindow} onToggleSidebar={panels.sidebar.toggle}
         toggleTerminal={panels.terminal.toggle} toggleNeovim={panels.editor.toggle} toggleGit={panels.git.toggle}
        mcpAgentActivity={mcpAgentActivity} fileEditCount={fileEditCount}
        sidebarOpen={panels.sidebar.open} terminalOpen={panels.terminal.open}
        terminalMounted={panels.terminal.mounted} neovimOpen={panels.editor.open}
        editorMounted={panels.editor.mounted} gitOpen={panels.git.open}
         panelOrder={panels.panelOrder} reorderPanels={panels.reorderPanels}
        gitMounted={panels.git.mounted} focusedPanel={panels.focused}
        sidebarResize={panels.sidebar.resize} sidePanelResize={panels.sidePanel.resize}
        terminalResize={panels.terminal.resize} searchBarOpen={modalState.modals.searchBar}
        searchMatchIds={modalState.searchMatchIds} activeSearchMatchId={modalState.activeSearchMatchId}
        mobileSidebarOpen={mobile.sidebarOpen} mobileInputHidden={mobile.inputHidden}
        isBookmarked={isBookmarked} toggleBookmark={toggleBookmark}
        handleSend={handlers.handleSend} handleAbort={handlers.handleAbort}
        handleCommand={handlers.handleCommand} handlePermissionReply={handlers.handlePermissionReply}
        handleQuestionReply={handlers.handleQuestionReply}
        handleQuestionDismiss={handlers.handleQuestionDismiss}
        handleSelectSession={handlers.handleSelectSession} handleNewSession={handlers.handleNewSession}
        handleSwitchProject={handlers.handleSwitchProject} handleAgentChange={handlers.handleAgentChange}
        handleRunnerChange={handleRunnerChange}
        handleEffortChange={(effort) => setRunnerSetting({ effort })}
        handlePermissionChange={(permission) => setRunnerSetting({ permission })}
        handleSearchMatchesChanged={callbacks.handleSearchMatchesChanged}
        handleScrollDirection={mobile.handleScrollDirection}
        handlePromptContentChange={mobile.handlePromptContentChange}
        loadOlderMessages={loadOlderMessages}
        openAddProject={openAddProject} openModelPicker={openModelPicker}
        openAgentPicker={openAgentPicker} openMemory={openMemoryActive}
        openCommandPalette={openCmdPalette} closeSearchBar={closeSearchBar}
        debugOpen={panels.debug.open} closeDebug={panels.debug.close}
        closeTerminal={panels.terminal.close} closeNeovim={panels.editor.close} closeGit={panels.git.close}
        closeMobileSidebar={mobile.closeSidebar} toggleMobileSidebar={mobile.toggleSidebar}
        focusSidebar={panels.focusSidebar} focusChat={panels.focusChat} focusSide={panels.focusSide}
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
        onCommand={handlers.handleCommand} onNewSession={handlers.handleNewSession}
        onSelectSession={handlers.handleSelectSession} onSend={handlers.handleSend}
        onModelSelected={handlers.handleModelSelected} onAgentChange={handlers.handleAgentChange}
        onContextSubmit={callbacks.handleContextSubmit} onThemeApplied={callbacks.handleThemeApplied}
        onRestoreWorkspace={callbacks.handleRestoreWorkspace}
        buildCurrentSnapshot={callbacks.buildCurrentSnapshot}
        onCompactContext={onCompactCtx} onAutonomyChange={callbacks.onAutonomyChange}
        onDismissSignal={callbacks.onDismissSignal}
        onQuickSetupDailyCopilot={handleRunAssistantPulse}
        onQuickSetupDailySummary={handleRunAssistantPulse}
        onQuickUpgradeAutonomy={handleRunAssistantPulse}
        toggleSidebar={panels.sidebar.toggle} toggleTerminal={panels.terminal.toggle}
        toggleNeovim={panels.editor.toggle} toggleGit={panels.git.toggle}
        selectedModel={model.selectedModel} selectedAgent={model.selectedAgent}
        themeMode={themeMode} setThemeMode={setThemeMode}
        appearance={appearance} setAppearance={setAppearanceState} fileEditCount={fileEditCount}
        allPermissions={allPermissions} allQuestions={allQuestions}
        sidebarOpen={panels.sidebar.open} terminalOpen={panels.terminal.open}
        neovimOpen={panels.editor.open} gitOpen={panels.git.open}
        liveActivityEvents={sse.liveActivityEvents} watcherStatus={watcherStatus}
        assistantSignals={assistant.assistantSignals} autonomyMode={assistant.autonomyMode}
        missionCache={assistant.missionCache} routineCache={assistant.routineCache}
        delegatedWorkCache={assistant.delegatedWorkCache}
        activeMemoryItems={assistant.activeMemoryItems}
        workspaceCache={assistant.workspaceCache} resumeBriefing={assistant.resumeBriefing}
        latestDailySummary={assistant.latestDailySummary}
        activeWorkspaceName={assistant.activeWorkspaceName}
        personalMemoryForInbox={callbacks.personalMemoryForInbox}
        memoryFilterActive={modalState.memoryFilterActive}
        openMemoryAll={openMemoryAll}
        splitViewSecondaryId={modalState.splitViewSecondaryId}
        setSplitViewSecondaryId={modalState.setSplitViewSecondaryId}
        clearPermission={clearPermission} clearQuestion={clearQuestion}
        onOpenSkillsUpload={() => setSkillsUploadOpen(true)}
      />
      <ToastContainer toasts={toasts} onDismiss={removeToast} />
      <MobileDock
        activePanel={mobile.activePanel} panelsMounted={mobile.panelsMounted}
        togglePanel={mobile.togglePanel} inputHidden={mobile.inputHidden}
        handleComposeButtonTap={mobile.handleComposeButtonTap}
        dockCollapsed={mobile.dockCollapsed} expandDock={mobile.expandDock}
        assistantCenterOpen={modalState.modals.assistantCenter}
        onOpenAssistantCenter={openAssistantCenter} onOpenCommandPalette={openCmdPalette}
        activeSessionId={activeSessionId} activeProject={activeProject}
        mcpEditorOpenPath={mcpEditorOpenPath} mcpEditorOpenLine={mcpEditorOpenLine}
        mcpAgentActivity={mcpAgentActivity}
        onError={callbacks.handlePanelError} onSendToAI={handlers.handleSend}
        isKanbanView={isKanbanView} onToggleKanban={toggleKanbanView}
      />
      {skillsUploadOpen && <SkillsUploadModal onClose={() => setSkillsUploadOpen(false)} />}
    </div>
    </EditorOpenProvider>
  );
}
