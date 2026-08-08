import React, { useCallback, useMemo } from "react";
import { ChatSidebar } from "./ChatSidebar";
import { MobileChatView } from "./MobileChatView";
import { StatusBar } from "./StatusBar";
import { KanbanView } from "./kanban/KanbanView";
import { useIsMobile } from "./hooks/useIsMobile";
import { DesktopShell } from "./workspace/DesktopShell";
import type { DesktopShellProps } from "./workspace/DesktopShell";

import type { SessionStatus } from "./hooks/sse/types";
import type { ShellSurface } from "./hooks/useSidebarState";
import type { SessionStats } from "./api";

/**
 * The chat surface.
 *
 * On desktop this is a thin shim: everything right of the sidebar is the
 * workspace, and the body below is reached only by mobile and by the board,
 * which takes over the whole area as the page it is.
 */
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
  handleModelSelected: (modelId: string, providerId: string) => void;
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
  watcherStatus: any;
  presenceClients?: any[];
  contextLimit: number | null;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
  onToggleSidebar: () => void;
  // Shell chrome
  sidebarOpen: boolean;
  focusedPanel: ShellSurface;
  sidebarResize: any;
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
  closeMobileSidebar: () => void;
  toggleMobileSidebar: () => void;
  focusSidebar: () => void;
  focusChat: () => void;
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
  /**
   * Everything the desktop workspace needs, assembled by ChatLayout. Absent on
   * mobile, which keeps the fixed panels and MobileDock untouched.
   */
  workspace?: Omit<DesktopShellProps, "sidebar" | "sidebarVisible" | "sidebarWidth" | "sidebarResizeHandle">;
}

export const ChatMainArea: React.FC<ChatMainAreaProps> = React.memo(function ChatMainArea(p) {
  const isMobile = useIsMobile();

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

  const chatHeader = <StatusBar project={p.activeProject} stats={p.stats} connectionStatus={p.connectionStatus} sidebarOpen={p.sidebarOpen} watcherStatus={p.watcherStatus} presenceClients={p.presenceClients} contextLimit={p.contextLimit} sessionTitle={sessionTitle} showSidebarToggle={!p.sidebarOpen} onToggleSidebar={p.onToggleSidebar} onOpenCommandPalette={p.openCommandPalette} onOpenWatcher={p.onOpenWatcher} onOpenContextWindow={p.onOpenContextWindow} />;

  const sidebarProps = {
    projects: p.appState.projects,
    activeProject: p.activeProjectIndex,
    activeSessionId: p.activeSessionId,
    isSessionBusy: p.isSessionBusy,
    busyKey: p.busyKey,
    onSelectSession: p.handleSelectSession,
    onNewSession: p.handleNewSession,
    onSwitchProject: p.handleSwitchProject,
    onOpenAddProject: p.openAddProject,
    isMobileOpen: p.mobileSidebarOpen,
    onClose: p.closeMobileSidebar,
    isKanbanView: p.isKanbanView,
    onToggleKanban: p.onToggleKanban,
    onToggleSidebar: p.onToggleSidebar,
    sessionTaskLinks: p.sessionTaskLinks,
    onOpenKanbanTask: p.onOpenKanbanTask,
  };

  // The desktop redesign: everything right of the sidebar becomes panes. The
  // board keeps taking over the whole area, as it did before — it is a page,
  // not a widget. Mobile falls through to the original layout untouched.
  if (!isMobile && !p.isKanbanView && p.workspace) {
    return (
      <DesktopShell
        {...p.workspace}
        sidebar={sidebarProps}
        sidebarVisible={p.sidebarOpen}
        sidebarWidth={p.sidebarResize.size}
        sidebarResizeHandle={p.sidebarResize.handleProps}
      />
    );
  }

  return (
    <div className="chat-content" data-surface="chat">
      {/* Sidebar */}
      {p.sidebarOpen && (
        <>
          <div
            style={{ width: p.sidebarResize.size, flexShrink: 0 }}
            className={p.focusedPanel !== "sidebar" ? "panel-dimmed" : ""}
            onMouseDown={p.focusSidebar}
            onFocus={p.focusSidebar}
          >
            <ChatSidebar {...sidebarProps} />
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
        <MobileChatView
          p={p}
          header={chatHeader}
          sessionTitle={sessionTitle}
          activeMemoryLabels={activeMemoryLabels}
          onOpenSession={handleOpenSession}
        />
      )}

    </div>
  );
});
