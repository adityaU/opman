import React from "react";
import { MessageTimeline } from "./MessageTimeline";
import { PromptInput } from "./PromptInput";
import { PermissionDock } from "./PermissionDock";
import { QuestionDock } from "./QuestionDock";
import { SearchBar } from "./SearchBar";
import { Sparkles, Command, WifiOff, ChevronDown } from "lucide-react";
import type { ChatMainAreaProps } from "./ChatMainArea";

/**
 * The single-transcript chat surface: header pill, timeline, docks, composer.
 *
 * Only mobile renders this. Desktop replaces it wholesale with the workspace,
 * where a transcript is a pane rather than the page — which is why nothing
 * here knows about panes, and why the composer's controls are the shell's
 * rather than any pane's.
 */
export interface MobileChatViewProps {
  readonly p: ChatMainAreaProps;
  /** The status bar, built once by the parent and shared with the board. */
  readonly header: React.ReactNode;
  readonly sessionTitle: string | null;
  readonly activeMemoryLabels: string[];
  readonly onOpenSession: (sessionId: string) => void;
}

export const MobileChatView: React.FC<MobileChatViewProps> = function MobileChatView({
  p,
  header,
  sessionTitle,
  activeMemoryLabels,
  onOpenSession,
}) {
  return (
  <div
    className={`chat-main${p.focusedPanel !== "chat" ? " panel-dimmed" : ""}`}
    onMouseDown={p.focusChat}
    onFocus={p.focusChat}
  >
    {header}
    {/* Mobile floating status pill */}
    <div className="chat-mobile-header">
      <button
        className="mobile-status-pill"
        onClick={p.toggleMobileSidebar}
        aria-label={p.mobileSidebarOpen ? "Close sidebar" : "Open sessions"}
      >
        <span className="mobile-project-glyph" aria-hidden="true">
          <Sparkles size={15} className="mobile-pill-icon" />
        </span>
        <span className="mobile-project-copy">
          <span className="mobile-project-label">Project</span>
          <span className="mobile-project-name">{p.activeProject?.name || "opman"}</span>
        </span>
        <span className="mobile-project-session">
          {sessionTitle || "New session"}
        </span>
        {p.sessionStatus.type !== "idle" && <span className="mobile-pill-busy" />}
        {p.connectionStatus && p.connectionStatus !== "connected" && (
          <span className={`mobile-pill-connection mobile-pill-connection-${p.connectionStatus}`}>
            <WifiOff size={12} />
          </span>
        )}
        <ChevronDown size={15} className="mobile-project-chevron" aria-hidden="true" />
      </button>
      <button className="mobile-cmd-btn" onClick={p.openCommandPalette} aria-label="Open command palette">
        <span className="mobile-cmd-icon" aria-hidden="true"><Command size={15} /></span>
        <span className="mobile-cmd-label">Commands</span>
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
      isSending={p.sending}
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
      onOpenSession={onOpenSession}
    />

    {/* Permission & question docks — always visible, independent of mobile input */}
    {p.allPermissions.length > 0 && (
      <PermissionDock
        permissions={p.allPermissions}
        activeSessionId={p.activeSessionId}
        onReply={p.handlePermissionReply}
        onGoToSession={onOpenSession}
      />
    )}
    {p.allQuestions.length > 0 && (
      <QuestionDock
        questions={p.allQuestions}
        activeSessionId={p.activeSessionId}
        onReply={p.handleQuestionReply}
        onDismiss={p.handleQuestionDismiss}
        onGoToSession={onOpenSession}
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
        // A new session is created lazily on the first send. Keep the
        // composer and runner/model controls usable while sessionId is null.
        disabled={!p.appState}
        sessionId={p.activeSessionId}
        currentModel={p.currentModel}
        selectedModel={p.selectedModel}
        onModelSelected={p.handleModelSelected}
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
      />
    </div>
  </div>
  );
};
