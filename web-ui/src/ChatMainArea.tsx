import React, { Suspense, lazy, useCallback, useMemo } from "react";
import { ChatSidebar } from "./ChatSidebar";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { PermissionDock } from "./PermissionDock";
import { QuestionDock } from "./QuestionDock";
import { TerminalPanel } from "./TerminalPanel";
import { SearchBar } from "./SearchBar";
import { X, FileCode, GitBranch, Sparkles, Command } from "lucide-react";

const CodeEditorPanel = lazy(() => import("./code-editor"));
const GitPanel = lazy(() => import("./git-panel"));

export interface ChatMainAreaProps {
  appState: any;
  activeProject: any;
  activeSessionId: string | null;
  sessionStatus: "idle" | "busy";
  messages: any[];
  busySessions: any;
  isLoadingMessages: boolean;
  isLoadingOlder: boolean;
  hasOlderMessages: boolean;
  totalMessageCount: number;
  subagentMessages: any;
  defaultModelDisplay: string | null;
  selectedModel: any;
  selectedAgent: string;
  sending: boolean;
  currentModel: string | null;
  allPermissions: any[];
  allQuestions: any[];
  activeMemoryItems: any[];
  mcpEditorOpenPath: string | null;
  mcpEditorOpenLine: number | null;
  mcpAgentActivity: Map<string, any>;
  fileEditCount: number;
  // Panel state
  sidebarOpen: boolean;
  terminalOpen: boolean;
  terminalMounted: boolean;
  neovimOpen: boolean;
  editorMounted: boolean;
  gitOpen: boolean;
  gitMounted: boolean;
  focusedPanel: "sidebar" | "chat" | "side";
  sidebarResize: any;
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
  handleSend: (text: string, images?: any[]) => Promise<void>;
  handleAbort: () => Promise<void>;
  handleCommand: (command: string, args?: string) => Promise<void>;
  handlePermissionReply: (requestId: string, reply: "once" | "always" | "reject") => Promise<void>;
  handleQuestionReply: (requestId: string, answers: string[][]) => Promise<void>;
  handleQuestionDismiss: (requestId: string) => Promise<void>;
  handleSelectSession: (sessionId: string, projectIdx: number) => Promise<void>;
  handleNewSession: () => Promise<void>;
  handleSwitchProject: (index: number) => Promise<void>;
  handleAgentChange: (agentId: string) => Promise<void>;
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
  closeMobileSidebar: () => void;
  toggleMobileSidebar: () => void;
  focusSidebar: () => void;
  focusChat: () => void;
  focusSide: () => void;
  handlePanelError: (msg: string) => void;
}

export const ChatMainArea: React.FC<ChatMainAreaProps> = (p) => {
  const hasSidePanel = p.neovimOpen || p.gitOpen;

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
              activeProject={p.appState.active_project}
              activeSessionId={p.activeSessionId}
              busySessions={p.busySessions}
              onSelectSession={p.handleSelectSession}
              onNewSession={p.handleNewSession}
              onSwitchProject={p.handleSwitchProject}
              onOpenAddProject={p.openAddProject}
              isMobileOpen={p.mobileSidebarOpen}
              onClose={p.closeMobileSidebar}
            />
          </div>
          <div {...p.sidebarResize.handleProps} />
        </>
      )}

      {/* Main chat area */}
      <div
        className={`chat-main${p.focusedPanel !== "chat" ? " panel-dimmed" : ""}`}
        onMouseDown={p.focusChat}
        onFocus={p.focusChat}
      >
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
            {p.sessionStatus === "busy" && <span className="mobile-pill-busy" />}
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
          onOpenSession={(sid: string) => p.handleSelectSession(sid, p.appState?.active_project ?? 0)}
        />

        {/* Permission & question docks — always visible, independent of mobile input */}
        {p.allPermissions.length > 0 && (
          <PermissionDock permissions={p.allPermissions} activeSessionId={p.activeSessionId} onReply={p.handlePermissionReply} />
        )}
        {p.allQuestions.length > 0 && (
          <QuestionDock questions={p.allQuestions} activeSessionId={p.activeSessionId} onReply={p.handleQuestionReply} onDismiss={p.handleQuestionDismiss} />
        )}

        {/* Mobile input wrapper */}
        <div className={`mobile-input-wrapper${p.mobileInputHidden ? " mobile-input-hidden" : ""}`}>
          <PromptInput
            onSend={p.handleSend}
            onAbort={p.handleAbort}
            onCommand={p.handleCommand}
            onOpenModelPicker={p.openModelPicker}
            onOpenAgentPicker={p.openAgentPicker}
            isBusy={p.sessionStatus === "busy"}
            isSending={p.sending}
            disabled={!p.activeSessionId}
            sessionId={p.activeSessionId}
            currentModel={p.currentModel}
            currentAgent={p.selectedAgent}
            onAgentChange={p.handleAgentChange}
            activeMemoryLabels={p.activeMemoryItems.map((item: any) => item.label)}
            onOpenMemory={p.openMemory}
            onContentChange={p.handlePromptContentChange}
          />
        </div>

        {/* Terminal panel */}
        {p.terminalMounted && (
          <>
            <div {...p.terminalResize.handleProps} style={{ ...p.terminalResize.handleProps.style, display: p.terminalOpen ? undefined : "none" }} />
            <div style={{ height: p.terminalResize.size, flexShrink: 0, display: p.terminalOpen ? undefined : "none" }}>
              <TerminalPanel
                sessionId={p.activeSessionId}
                onClose={p.closeTerminal}
                visible={p.terminalOpen}
                mcpAgentActive={Array.from(p.mcpAgentActivity.keys()).some((t) => t.startsWith("web_terminal"))}
              />
            </div>
          </>
        )}
      </div>

      {/* Side panel: Editor or Git */}
      {(hasSidePanel || p.editorMounted || p.gitMounted) && (
        <>
          <div {...p.sidePanelResize.handleProps} style={{ ...p.sidePanelResize.handleProps.style, display: hasSidePanel ? undefined : "none" }} />
          <div
            className={`side-panel${p.focusedPanel !== "side" ? " panel-dimmed" : ""}`}
            style={{ width: p.sidePanelResize.size, flexShrink: 0, display: hasSidePanel ? undefined : "none" }}
            onMouseDown={p.focusSide}
            onFocus={p.focusSide}
          >
            {p.editorMounted && (
              <div className="side-panel-section" style={{ display: p.neovimOpen ? undefined : "none" }}>
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
              <div className="side-panel-section" style={{ display: p.gitOpen ? undefined : "none" }}>
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
          </div>
        </>
      )}
    </div>
  );
};
