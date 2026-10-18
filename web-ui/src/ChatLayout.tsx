import { ChatMainArea } from "./ChatMainArea";
import { ModalLayer } from "./ModalLayer";
import { MobileDock } from "./MobileDock";
import { ToastContainer } from "./ToastContainer";
import { SettingsPage } from "./settings-page";
import { EditorOpenProvider } from "./tool-call/EditorOpenContext";
import { AgentSessionOpenProvider } from "./tool-call/agent-manager";
import { StartupGate } from "./StartupGate";
import { useChatBase } from "./useChatBase";
import { useChatSession } from "./useChatSession";
import { useChatWorkspace } from "./useChatWorkspace";

export function ChatLayout() {
  const base = useChatBase();
  const session = useChatSession(base);
  const context = useChatWorkspace(session);
  const {
    hasStartedRef, sse, appState, messages, stats, sessionStatus,
    isLoadingMessages, isLoadingOlder, hasOlderMessages, totalMessageCount, watcherStatus, subagentMessages,
    fileEditCount, mcpAgentActivity, refreshState, clearPermission, clearQuestion, loadOlderMessages,
    isSessionBusy, busyKey, activeProjectIndex, activeProject, activeSessionId, allPermissions,
    allQuestions, themeMode, setThemeMode, appearance, setAppearanceState, modalState,
    toasts, addToast, removeToast, availableRunners, currentRunner, providers,
    isBookmarked, toggleBookmark, settings, leaveSettings, isKanbanView, boardProjectIndex,
    focusTaskId, setBoardProject, clearFocusTask, toggleKanbanView, handleOpenKanbanTask, sessionTaskLinks,
    sidebar, mobile, model, handleRunnerChange, sessionEngine, supportedEfforts,
    effortOptions, assistant, openModal, handlers, callbacks, closeModal,
    closeModalSilent, openAddProject, openModelPicker, openAgentPicker, workspaceProps, openKindHere,
    fileOpen, editorOpener, handleSelectSessionOrTarget, handleNewSessionOrTarget, agentSessionOpener, openMemoryActive,
    openMemoryAll, openCmdPalette, closeSearchBar, openWatcher, openCtxWindow, onCompactCtx,
  } = context;
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
  if (!hasStartedRef.current && (!startupReady || !liveReady || (activeSessionId && providers.loading) || !workspaceReady)) {
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
    <EditorOpenProvider value={editorOpener}>
    <AgentSessionOpenProvider value={agentSessionOpener}>
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
        onProjectChanged={refreshState}
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
        onProjectChanged={refreshState}
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
    </AgentSessionOpenProvider>
    </EditorOpenProvider>
  );
}
