import React, { useState, useCallback, useMemo, useEffect } from "react";
import { useSSE } from "./hooks/useSSE";
import { useKeyboard } from "./hooks/useKeyboard";
import { useToast } from "./hooks/useToast";
import { useProviders } from "./hooks/useProviders";
import { useBookmarks } from "./hooks/useBookmarks";
import { useModalState } from "./hooks/useModalState";
import type { ModalName } from "./hooks/useModalState";
import { usePanelState } from "./hooks/usePanelState";
import { useMobileState } from "./hooks/useMobileState";
import { useModelState } from "./hooks/useModelState";
import { useAssistantState } from "./hooks/useAssistantState";
import { useUrlRestore } from "./hooks/useUrlRestore";
import { useNotificationSignals } from "./hooks/useNotificationSignals";
import { usePulseActions } from "./hooks/usePulseActions";
import { useChatHandlers } from "./hooks/useChatHandlers";
import { useChatCallbacks } from "./hooks/useChatCallbacks";
import { buildKeyboardShortcuts } from "./chatLayoutKeyboard";
import { ChatMainArea } from "./ChatMainArea";
import { ModalLayer } from "./ModalLayer";
import { MobileDock } from "./MobileDock";
import { StatusBar } from "./StatusBar";
import { ToastContainer } from "./ToastContainer";
import { getPersistedThemeMode, applyThemeMode } from "./ThemeSelectorModal";
import type { ThemeMode } from "./ThemeSelectorModal";
import type { ThemeColors } from "./api";
import { applyThemeToCss } from "./utils/theme";
import { fetchTheme } from "./api";

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
    clearMcpEditorOpen, clearMcpTerminalFocus,
    addOptimisticMessage, loadOlderMessages,
  } = sse;

  // ── Derived app state ──
  const activeProject = appState ? appState.projects[appState.active_project] ?? null : null;
  const activeSessionId = activeProject?.active_session ?? null;
  const activeProjectIndex = appState?.active_project ?? 0;

  const allPermissions = useMemo(() => [...permissions, ...crossSessionPermissions], [permissions, crossSessionPermissions]);
  const allQuestions = useMemo(() => [...questions, ...crossSessionQuestions], [questions, crossSessionQuestions]);

  // ── Theme ──
  const [themeMode, setThemeMode] = useState<ThemeMode>(getPersistedThemeMode);
  useEffect(() => {
    applyThemeMode(themeMode);
    fetchTheme().then((colors) => { if (colors) applyThemeToCss(colors); });
  }, []);

  // ── Modals / Toast / Providers / Bookmarks ──
  const modalState = useModalState();
  const { toasts, addToast, removeToast } = useToast();
  const providers = useProviders();
  const { isBookmarked, toggleBookmark } = useBookmarks();

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

  // ── Model / Agent ──
  const model = useModelState(messages, providers);

  // ── URL restore/sync ──
  useUrlRestore({
    appState, activeSessionId,
    panels: {
      sidebarOpen: panels.sidebar.open, terminalOpen: panels.terminal.open,
      neovimOpen: panels.editor.open, gitOpen: panels.git.open,
    },
    setPanels, refreshState,
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
      onOpenInbox: () => modalState.open("inbox"),
      onOpenActivityFeed: () => modalState.open("activityFeed"),
      onOpenMissions: () => modalState.open("missions"),
      onOpenAssistantCenter: () => modalState.open("assistantCenter"),
    },
  );

  // ── Notification signals ──
  useNotificationSignals({
    activeSessionId, sessionStatus,
    autonomyMode: assistant.autonomyMode, watcherStatus,
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
  const handlers = useChatHandlers({
    activeSessionId, appState,
    selectedModel: model.selectedModel, selectedAgent: model.selectedAgent,
    sending: model.sending, activeMemoryItems: assistant.activeMemoryItems,
    setSending: model.setSending, setSelectedModel: model.setSelectedModel,
    setSelectedAgent: model.setSelectedAgent,
    setMobileInputHidden: mobile.setInputHidden,
    addToast, addOptimisticMessage, refreshState,
    clearPermission, clearQuestion,
    setMobileSidebarOpen: mobile.setSidebarOpen,
    openModal,
    toggleSidebar: panels.sidebar.toggle, toggleTerminal: panels.terminal.toggle,
    toggleNeovim: panels.editor.toggle, toggleGit: panels.git.toggle,
    toggleSplitView: () => modalState.toggle("splitView"),
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
  const openAddProject = useCallback(() => modalState.open("addProject"), [modalState]);
  const openModelPicker = useCallback(() => modalState.open("modelPicker"), [modalState]);
  const openAgentPicker = useCallback(() => modalState.open("agentPicker"), [modalState]);
  const openMemory = useCallback(() => modalState.open("memory"), [modalState]);
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
    return <div className="chat-loading"><div className="chat-loading-spinner" /><span>Connecting to opman...</span></div>;
  }

  return (
    <div className="chat-layout">
      {mobile.sidebarOpen && <div className="sidebar-overlay visible" onClick={mobile.closeSidebar} />}
      <ChatMainArea
        appState={appState} activeProject={activeProject} activeSessionId={activeSessionId}
        sessionStatus={sessionStatus} messages={messages} busySessions={busySessions}
        isLoadingMessages={isLoadingMessages} isLoadingOlder={isLoadingOlder}
        hasOlderMessages={hasOlderMessages} totalMessageCount={totalMessageCount}
        subagentMessages={subagentMessages} defaultModelDisplay={model.defaultModelDisplay}
        selectedModel={model.selectedModel} selectedAgent={model.selectedAgent}
        sending={model.sending} currentModel={model.currentModel}
        allPermissions={allPermissions} allQuestions={allQuestions}
        activeMemoryItems={assistant.activeMemoryItems}
        mcpEditorOpenPath={mcpEditorOpenPath} mcpEditorOpenLine={mcpEditorOpenLine}
        mcpAgentActivity={mcpAgentActivity} fileEditCount={fileEditCount}
        sidebarOpen={panels.sidebar.open} terminalOpen={panels.terminal.open}
        terminalMounted={panels.terminal.mounted} neovimOpen={panels.editor.open}
        editorMounted={panels.editor.mounted} gitOpen={panels.git.open}
        gitMounted={panels.git.mounted} focusedPanel={panels.focused}
        sidebarResize={panels.sidebar.resize} sidePanelResize={panels.sidePanel.resize}
        terminalResize={panels.terminal.resize} searchBarOpen={modalState.modals.searchBar}
        searchMatchIds={modalState.searchMatchIds} activeSearchMatchId={modalState.activeSearchMatchId}
        mobileSidebarOpen={mobile.sidebarOpen} mobileInputHidden={mobile.inputHidden}
        isBookmarked={isBookmarked} toggleBookmark={toggleBookmark}
        handleSend={handlers.handleSend} handleAbort={handlers.handleAbort}
        handleCommand={handlers.handleCommand} handlePermissionReply={handlers.handlePermissionReply}
        handleQuestionReply={handlers.handleQuestionReply}
        handleSelectSession={handlers.handleSelectSession} handleNewSession={handlers.handleNewSession}
        handleSwitchProject={handlers.handleSwitchProject} handleAgentChange={handlers.handleAgentChange}
        handleSearchMatchesChanged={callbacks.handleSearchMatchesChanged}
        handleScrollDirection={mobile.handleScrollDirection}
        handlePromptContentChange={mobile.handlePromptContentChange}
        loadOlderMessages={loadOlderMessages}
        openAddProject={openAddProject} openModelPicker={openModelPicker}
        openAgentPicker={openAgentPicker} openMemory={openMemory}
        openCommandPalette={openCmdPalette} closeSearchBar={closeSearchBar}
        closeTerminal={panels.terminal.close} closeNeovim={panels.editor.close} closeGit={panels.git.close}
        closeMobileSidebar={mobile.closeSidebar} toggleMobileSidebar={mobile.toggleSidebar}
        focusSidebar={panels.focusSidebar} focusChat={panels.focusChat} focusSide={panels.focusSide}
        handlePanelError={callbacks.handlePanelError}
      />
      <ModalLayer
        modals={modalState.modals} openModal={openModal} closeModal={closeModal}
        appState={appState} activeSessionId={activeSessionId} activeProject={activeProject}
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
        themeMode={themeMode} setThemeMode={setThemeMode} fileEditCount={fileEditCount}
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
        splitViewSecondaryId={modalState.splitViewSecondaryId}
        setSplitViewSecondaryId={modalState.setSplitViewSecondaryId}
        clearPermission={clearPermission} clearQuestion={clearQuestion}
      />
      <StatusBar
        project={activeProject} stats={stats} sessionStatus={sessionStatus}
        sidebarOpen={panels.sidebar.open} terminalOpen={panels.terminal.open}
        neovimOpen={panels.editor.open} gitOpen={panels.git.open}
        watcherStatus={watcherStatus} contextLimit={model.currentModelContextLimit}
        presenceClients={sse.presenceClients} activeWorkspaceName={assistant.activeWorkspaceName}
        activeMemoryItems={callbacks.personalMemoryForInbox}
        autonomyMode={assistant.autonomyMode} assistantPulse={assistant.assistantPulse}
        onRunAssistantPulse={handleRunAssistantPulse}
        onToggleSidebar={panels.sidebar.toggle} onToggleTerminal={panels.terminal.toggle}
        onToggleNeovim={panels.editor.toggle} onToggleGit={panels.git.toggle}
        onOpenCommandPalette={openCmdPalette} onOpenWatcher={openWatcher}
        onOpenContextWindow={openCtxWindow}
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
      />
    </div>
  );
}
