import React, { Suspense, lazy, useCallback, useEffect, useMemo, useState } from "react";
import { ChatSidebar } from "./ChatSidebar";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { PermissionDock } from "./PermissionDock";
import { QuestionDock } from "./QuestionDock";
import { SearchBar } from "./SearchBar";
import { StatusBar } from "./StatusBar";
import { X, FileCode, GitBranch, Sparkles, Command, WifiOff, Activity, Terminal as TerminalIcon } from "lucide-react";
import { KanbanView } from "./kanban/KanbanView";

import type { SessionStatus } from "./hooks/sse/types";
import type { SessionStats } from "./api";

const CodeEditorPanel = lazy(() => import("./code-editor"));
const GitPanel = lazy(() => import("./git-panel"));
const TerminalPanel = lazy(() => import("./TerminalPanel").then(m => ({ default: m.TerminalPanel })));
const DebugPanel = lazy(() => import("./DebugPanel").then(m => ({ default: m.DebugPanel })));

export interface ChatMainAreaProps {
  /** When true, the main area shows the Kanban board instead of the chat. */
  isKanbanView: boolean;
  onToggleKanban: () => void;
  appState: any;
  activeProject: any;
  activeProjectIndex: number;
  activeSessionId: string | null;
  sessionStatus: SessionStatus;
  /** Session token/cost stats — passed through to the prompt input's usage info button. */
  stats: SessionStats | null;
  connectionStatus?: "connected" | "reconnecting" | "disconnected";
  messages: any[];
  /** Stable callback — reads from ref, never changes identity. */
  isSessionBusy: (sid: string) => boolean;
  /** Serialized key — changes only when the set of busy IDs changes. */
  busyKey: string;
  isLoadingMessages: boolean;
  isLoadingOlder: boolean;
  hasOlderMessages: boolean;
  totalMessageCount: number;
  subagentMessages: any;
  defaultModelDisplay: string | null;
  selectedModel: any;
  selectedAgent: string;
  selectedRunner: string;
  availableRunners: string[];
  supportedEfforts: string[];
  effort: string | null;
  permission: string;
  sending: boolean;
  currentModel: string | null;
  activeMemoryItems: any[];
  allPermissions: any[];
  allQuestions: any[];
  mcpEditorOpenPath: string | null;
  mcpEditorOpenLine: number | null;
  mcpAgentActivity: Map<string, any>;
  fileEditCount: number;
  watcherStatus: any;
  presenceClients?: any[];
  activeWorkspaceName?: string | null;
  autonomyMode?: any;
  assistantPulse?: any;
  contextLimit: number | null;
  backend?: string;
  onRunAssistantPulse?: () => void;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
  onToggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  // Panel state
  sidebarOpen: boolean;
  terminalOpen: boolean;
  terminalMounted: boolean;
  /** Bumped when the input's "Attach terminal" button is clicked. */
  terminalAttachNonce?: number;
  /** Open an interactive terminal attached to the session's claude CLI agent. */
  onAttachTerminal?: () => void;
  neovimOpen: boolean;
  editorMounted: boolean;
  gitOpen: boolean;
  gitMounted: boolean;
  debugOpen: boolean;
  focusedPanel: "sidebar" | "chat" | "side";
  sidebarResize: any;
  panelOrder: string[];
  reorderPanels: (source: string, target: string) => void;
  sidePanelResize: any;
  terminalResize: any;
  // Search
  searchBarOpen: boolean;
  searchMatchIds: Set<string>;
  activeSearchMatchId: string | null;
  // Mobile
  mobileSidebarOpen: boolean;
  mobileInputHidden: boolean;
  // Bookmarks
  isBookmarked: (id: string) => boolean;
  toggleBookmark: (id: string, sessionId: string, role: string, preview: string) => void;
  // Callbacks
  handleSend: (text: string, images?: any[]) => Promise<boolean>;
  handleAbort: () => Promise<void>;
  handleCommand: (command: string, args?: string) => Promise<void>;
  handlePermissionReply: (requestId: string, reply: "once" | "always" | "reject") => Promise<void>;
  handleQuestionReply: (requestId: string, answers: string[][]) => Promise<void>;
  handleQuestionDismiss: (requestId: string) => Promise<void>;
  handleSelectSession: (sessionId: string, projectIdx: number) => void;
  handleNewSession: () => Promise<void>;
  handleSwitchProject: (index: number) => Promise<void>;
  handleAgentChange: (agentId: string) => Promise<void>;
  handleRunnerChange: (runner: string) => void;
  handleEffortChange: (effort: string | null) => void;
  handlePermissionChange: (permission: string) => void;
  handleSearchMatchesChanged: (matchIds: Set<string>, activeId: string | null) => void;
  handleScrollDirection: (direction: "up" | "down") => void;
  handlePromptContentChange: (hasContent: boolean) => void;
  loadOlderMessages: () => Promise<boolean>;
  openAddProject: () => void;
  openModelPicker: () => void;
  openAgentPicker: () => void;
  openMemory: () => void;
  openCommandPalette: () => void;
  closeSearchBar: () => void;
  closeTerminal: () => void;
  closeNeovim: () => void;
  closeGit: () => void;
  closeDebug: () => void;
  closeMobileSidebar: () => void;
  toggleMobileSidebar: () => void;
  focusSidebar: () => void;
  focusChat: () => void;
  focusSide: () => void;
  handlePanelError: (msg: string) => void;
  /** session_id → originating kanban task/lane, for the active project's board. */
  sessionTaskLinks?: Map<string, import("./sidebar/useSessionTaskLinks").SessionTaskLink>;
  /** Open the originating kanban task (back-link from a session). */
  onOpenKanbanTask?: (taskId: string) => void;
  /** Task whose editor the board should open on mount (`?task=<id>`). */
  focusTaskId?: string | null;
  /** Clear the focus-task URL param once the board has consumed it. */
  clearFocusTask?: () => void;
  /** Project the board shows (`?project`), or null to fall back to the active project. */
  boardProjectIndex?: number | null;
  /** Switch the board's project, syncing it to the URL. */
  onSelectBoardProject?: (projectIndex: number) => void;
}

export const ChatMainArea: React.FC<ChatMainAreaProps> = React.memo(function ChatMainArea(p) {
  const [activeRightPanel, setActiveRightPanel] = useState("editor");
  const visibleRightPanels = [p.terminalOpen ? "terminal" : null, p.neovimOpen ? "editor" : null, p.gitOpen ? "git" : null, p.debugOpen ? "debug" : null].filter((id): id is string => Boolean(id));
  useEffect(() => {
    if (p.mcpEditorOpenPath && p.neovimOpen) { setActiveRightPanel("editor"); return; }
    if (!visibleRightPanels.includes(activeRightPanel)) setActiveRightPanel(visibleRightPanels[0] || "editor");
  }, [p.mcpEditorOpenPath, p.neovimOpen, activeRightPanel, visibleRightPanels.join("|")]);
  const hasSidePanel = p.terminalOpen || p.terminalMounted || p.neovimOpen || p.gitOpen || p.debugOpen;

  // Stable callback: navigate to session within active project
  const handleOpenSession = useCallback(
    (sid: string) => p.handleSelectSession(sid, p.activeProjectIndex),
    [p.handleSelectSession, p.activeProjectIndex],
  );

  // Stable memory labels array — avoid recreating every render
  const activeMemoryLabels = useMemo(
    () => p.activeMemoryItems.map((item: any) => item.label),
    [p.activeMemoryItems],
  );

  const sessionTitle = useMemo(() => {
    if (!p.activeSessionId) return null;
    return p.activeProject?.sessions?.find((session: any) => session.id === p.activeSessionId)?.title || null;
  }, [p.activeProject, p.activeSessionId]);

  const chatHeader = <StatusBar project={p.activeProject} stats={p.stats} connectionStatus={p.connectionStatus} sidebarOpen={p.sidebarOpen} terminalOpen={p.terminalOpen} neovimOpen={p.neovimOpen} gitOpen={p.gitOpen} watcherStatus={p.watcherStatus} presenceClients={p.presenceClients} activeWorkspaceName={p.activeWorkspaceName} contextLimit={p.contextLimit} sessionTitle={sessionTitle} showSidebarToggle={!p.sidebarOpen} onToggleSidebar={p.onToggleSidebar} onToggleTerminal={p.toggleTerminal} onToggleNeovim={p.toggleNeovim} onToggleGit={p.toggleGit} onOpenCommandPalette={p.openCommandPalette} onOpenWatcher={p.onOpenWatcher} onOpenContextWindow={p.onOpenContextWindow} />;

  return (
    <div className="chat-content">
      {/* Sidebar */}
      {p.sidebarOpen && (
        <>
          <div
            style={{ width: p.sidebarResize.size, flexShrink: 0 }}
            className={p.focusedPanel !== "sidebar" ? "panel-dimmed" : ""}
            onMouseDown={p.focusSidebar}
            onFocus={p.focusSidebar}
          >
            <ChatSidebar
              projects={p.appState.projects}
              activeProject={p.activeProjectIndex}
              activeSessionId={p.activeSessionId}
              isSessionBusy={p.isSessionBusy}
              busyKey={p.busyKey}
              onSelectSession={p.handleSelectSession}
              onNewSession={p.handleNewSession}
              onSwitchProject={p.handleSwitchProject}
              onOpenAddProject={p.openAddProject}
              isMobileOpen={p.mobileSidebarOpen}
              onClose={p.closeMobileSidebar}
              isKanbanView={p.isKanbanView}
              onToggleKanban={p.onToggleKanban}
              onToggleSidebar={p.onToggleSidebar}
              sessionTaskLinks={p.sessionTaskLinks}
              onOpenKanbanTask={p.onOpenKanbanTask}
            />
          </div>
          <div {...p.sidebarResize.handleProps} />
        </>
      )}

      {/* Main area: Kanban board takes over the chat slot when active */}
      {p.isKanbanView ? (
        <div
          className={`chat-main${p.focusedPanel !== "chat" ? " panel-dimmed" : ""}`}
          onMouseDown={p.focusChat}
          onFocus={p.focusChat}
        >
          {chatHeader}
          <KanbanView
            projects={p.appState.projects}
            projectIndex={p.boardProjectIndex ?? p.activeProjectIndex}
            onSelectProject={p.onSelectBoardProject ?? (() => {})}
            onOpenSession={p.handleSelectSession}
            onError={(msg) => p.handlePanelError(msg)}
            focusTaskId={p.focusTaskId}
            onFocusTaskConsumed={p.clearFocusTask}
          />
        </div>
      ) : (
      <div
        className={`chat-main${p.focusedPanel !== "chat" ? " panel-dimmed" : ""}`}
        onMouseDown={p.focusChat}
        onFocus={p.focusChat}
      >
        {chatHeader}
        {/* Mobile floating status pill */}
        <div className="chat-mobile-header">
          <button
            className="mobile-status-pill"
            onClick={p.toggleMobileSidebar}
            aria-label={p.mobileSidebarOpen ? "Close sidebar" : "Open sessions"}
          >
            <Sparkles size={14} className="mobile-pill-icon" />
            <span className="mobile-project-name">
              {p.activeProject?.name || "opman"}
            </span>
            {p.sessionStatus.type !== "idle" && <span className="mobile-pill-busy" />}
            {p.connectionStatus && p.connectionStatus !== "connected" && (
              <span className={`mobile-pill-connection mobile-pill-connection-${p.connectionStatus}`}>
                <WifiOff size={12} />
              </span>
            )}
          </button>
          <button className="mobile-cmd-btn" onClick={p.openCommandPalette} aria-label="Open command palette">
            <Command size={14} />
          </button>
        </div>

        {/* In-session search bar */}
        {p.searchBarOpen && (
          <SearchBar messages={p.messages} onClose={p.closeSearchBar} onMatchesChanged={p.handleSearchMatchesChanged} />
        )}

        {/* Message timeline */}
        <MessageTimeline
          messages={p.messages}
          sessionStatus={p.sessionStatus}
          activeSessionId={p.activeSessionId}
          isLoadingMessages={p.isLoadingMessages}
          isLoadingOlder={p.isLoadingOlder}
          hasOlderMessages={p.hasOlderMessages}
          totalMessageCount={p.totalMessageCount}
          onLoadOlder={p.loadOlderMessages}
          appState={p.appState}
          defaultModel={p.defaultModelDisplay}
          onSendPrompt={p.handleSend}
          subagentMessages={p.subagentMessages}
          searchMatchIds={p.searchMatchIds}
          activeSearchMatchId={p.activeSearchMatchId}
          isBookmarked={p.isBookmarked}
          onToggleBookmark={p.toggleBookmark}
          onScrollDirection={p.handleScrollDirection}
          onOpenSession={handleOpenSession}
        />

        {/* Permission & question docks — always visible, independent of mobile input */}
        {p.allPermissions.length > 0 && (
          <PermissionDock
            permissions={p.allPermissions}
            activeSessionId={p.activeSessionId}
            onReply={p.handlePermissionReply}
            onGoToSession={handleOpenSession}
          />
        )}
        {p.allQuestions.length > 0 && (
          <QuestionDock
            questions={p.allQuestions}
            activeSessionId={p.activeSessionId}
            onReply={p.handleQuestionReply}
            onDismiss={p.handleQuestionDismiss}
            onGoToSession={handleOpenSession}
          />
        )}

        {/* Mobile input wrapper */}
        <div className={`mobile-input-wrapper${p.mobileInputHidden ? " mobile-input-hidden" : ""}`}>
          <PromptInput
            onSend={p.handleSend}
            onAbort={p.handleAbort}
            onCommand={p.handleCommand}
            onOpenModelPicker={p.openModelPicker}
            onOpenAgentPicker={p.openAgentPicker}
            isBusy={p.sessionStatus.type !== "idle"}
            isSending={p.sending}
            disabled={!p.activeSessionId}
            sessionId={p.activeSessionId}
            currentModel={p.currentModel}
            currentAgent={p.selectedAgent}
            onAgentChange={p.handleAgentChange}
            currentRunner={p.selectedRunner}
            availableRunners={p.availableRunners}
            onRunnerChange={p.handleRunnerChange}
            supportedEfforts={p.supportedEfforts}
            effort={p.effort}
            permission={p.permission}
            onEffortChange={p.handleEffortChange}
            onPermissionChange={p.handlePermissionChange}
            stats={p.stats}
            activeMemoryLabels={activeMemoryLabels}
            onOpenMemory={p.openMemory}
            onContentChange={p.handlePromptContentChange}
            backend={p.appState?.backend}
            onAttachTerminal={p.onAttachTerminal}
          />
        </div>
      </div>
      )}

      {/* Side panel: Editor or Git */}
      {(hasSidePanel || p.editorMounted || p.gitMounted || p.terminalMounted) && (
        <>
          <div {...p.sidePanelResize.handleProps} style={{ ...p.sidePanelResize.handleProps.style, display: hasSidePanel ? undefined : "none" }} />
          <div
            className={`right-panel-stack${p.focusedPanel !== "side" ? " panel-dimmed" : ""}`}
            style={{ width: p.sidePanelResize.size, flexShrink: 0, display: hasSidePanel ? undefined : "none" }}
            onMouseDown={p.focusSide}
            onFocus={p.focusSide}
          >
             <div className="right-panel-tabs" role="tablist">
               {visibleRightPanels.map((id) => (
                 <button key={id} type="button" role="tab" aria-selected={activeRightPanel === id} className={activeRightPanel === id ? "active" : ""} onClick={() => setActiveRightPanel(id)}>
                   {id === "editor" ? "Files" : id.charAt(0).toUpperCase() + id.slice(1)}
                 </button>
               ))}
             </div>
            {p.terminalMounted && (
              <div className="side-panel-section right-panel-card" style={{ display: p.terminalOpen && activeRightPanel === "terminal" ? undefined : "none" }}>
                <div className="side-panel-header"><TerminalIcon size={14} /><span>Terminal</span><button className="side-panel-close" onClick={p.closeTerminal} aria-label="Close terminal panel"><X size={14} /></button></div>
                <div className="side-panel-body">
                  <Suspense fallback={null}><TerminalPanel sessionId={p.activeSessionId} projectPath={p.activeProject?.path ?? null} onClose={p.closeTerminal} visible={p.terminalOpen} attachNonce={p.terminalAttachNonce} attachKind="claude-attach" mcpAgentActive={Array.from(p.mcpAgentActivity.keys()).some((t) => t.startsWith("web_terminal"))} /></Suspense>
                </div>
              </div>
            )}
            {p.editorMounted && (
              <div className="side-panel-section right-panel-card" style={{ display: p.neovimOpen && activeRightPanel === "editor" ? undefined : "none" }}>
                <div className="side-panel-header">
                  <FileCode size={14} />
                  <span>Editor</span>
                  {Array.from(p.mcpAgentActivity.keys()).some((t) => t.startsWith("web_editor")) && (
                    <span className="mcp-agent-indicator" title="AI agent active"><span className="mcp-agent-dot" /></span>
                  )}
                  <button className="side-panel-close" onClick={p.closeNeovim} aria-label="Close editor panel"><X size={14} /></button>
                </div>
                <div className="side-panel-body">
                  <Suspense fallback={null}>
                    <CodeEditorPanel
                      focused={p.neovimOpen && !p.gitOpen}
                      openFilePath={p.mcpEditorOpenPath}
                      openLine={p.mcpEditorOpenLine}
                      projectPath={p.activeProject?.path}
                      sessionId={p.activeSessionId}
                      onError={p.handlePanelError}
                    />
                  </Suspense>
                </div>
              </div>
            )}
            {p.gitMounted && (
              <div className="side-panel-section right-panel-card" style={{ display: p.gitOpen && activeRightPanel === "git" ? undefined : "none" }}>
                <div className="side-panel-header">
                  <GitBranch size={14} />
                  <span>Git</span>
                  <button className="side-panel-close" onClick={p.closeGit} aria-label="Close git panel"><X size={14} /></button>
                </div>
                <div className="side-panel-body">
                  <Suspense fallback={null}>
                    <GitPanel focused={p.gitOpen} projectPath={p.activeProject?.path} onError={p.handlePanelError} onSendToAI={p.handleSend} />
                  </Suspense>
                </div>
              </div>
            )}
            {p.debugOpen && (
              <div className="side-panel-section right-panel-card" style={{ flex: 1, display: activeRightPanel === "debug" ? "flex" : "none", flexDirection: "column" }}>
                <div className="side-panel-header">
                  <Activity size={14} />
                  <span>Debug</span>
                  <button className="side-panel-close" onClick={p.closeDebug} aria-label="Close debug panel"><X size={14} /></button>
                </div>
                <div className="side-panel-body">
                  <Suspense fallback={null}>
                    <DebugPanel />
                  </Suspense>
                </div>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
});
