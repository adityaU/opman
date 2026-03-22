import React, { Suspense, lazy, useCallback } from "react";

// Lazy-load all modals — they are conditionally rendered and rarely all open at once
const CommandPalette = lazy(() => import("./CommandPalette").then(m => ({ default: m.CommandPalette })));
const ModelPickerModal = lazy(() => import("./ModelPickerModal").then(m => ({ default: m.ModelPickerModal })));
const AgentPickerModal = lazy(() => import("./AgentPickerModal").then(m => ({ default: m.AgentPickerModal })));
const ThemeSelectorModal = lazy(() => import("./ThemeSelectorModal").then(m => ({ default: m.ThemeSelectorModal })));
const CheatsheetModal = lazy(() => import("./CheatsheetModal").then(m => ({ default: m.CheatsheetModal })));
const TodoPanelModal = lazy(() => import("./TodoPanelModal").then(m => ({ default: m.TodoPanelModal })));
const SessionSelectorModal = lazy(() => import("./SessionSelectorModal").then(m => ({ default: m.SessionSelectorModal })));
const ContextInputModal = lazy(() => import("./ContextInputModal").then(m => ({ default: m.ContextInputModal })));
const SettingsModal = lazy(() => import("./SettingsModal").then(m => ({ default: m.SettingsModal })));
const WatcherModal = lazy(() => import("./WatcherModal").then(m => ({ default: m.WatcherModal })));
const AddProjectModal = lazy(() => import("./AddProjectModal").then(m => ({ default: m.AddProjectModal })));
const ContextWindowPanel = lazy(() => import("./ContextWindowPanel").then(m => ({ default: m.ContextWindowPanel })));
const DiffReviewPanel = lazy(() => import("./DiffReviewPanel").then(m => ({ default: m.DiffReviewPanel })));
const CrossSessionSearchModal = lazy(() => import("./CrossSessionSearchModal").then(m => ({ default: m.CrossSessionSearchModal })));
const SplitView = lazy(() => import("./SplitView").then(m => ({ default: m.SplitView })));

const SessionGraph = lazy(() => import("./SessionGraph").then(m => ({ default: m.SessionGraph })));
const SessionDashboard = lazy(() => import("./SessionDashboard").then(m => ({ default: m.SessionDashboard })));
const ActivityFeed = lazy(() => import("./ActivityFeed").then(m => ({ default: m.ActivityFeed })));
const NotificationPrefsModal = lazy(() => import("./NotificationPrefsModal").then(m => ({ default: m.NotificationPrefsModal })));
const InboxModal = lazy(() => import("./InboxModal").then(m => ({ default: m.InboxModal })));
const AutonomyModal = lazy(() => import("./AutonomyModal").then(m => ({ default: m.AutonomyModal })));
const MemoryModal = lazy(() => import("./MemoryModal").then(m => ({ default: m.MemoryModal })));
const RoutinesModal = lazy(() => import("./RoutinesModal").then(m => ({ default: m.RoutinesModal })));
const DelegationBoardModal = lazy(() => import("./DelegationBoardModal").then(m => ({ default: m.DelegationBoardModal })));
const AssistantCenterModal = lazy(() => import("./AssistantCenterModal").then(m => ({ default: m.AssistantCenterModal })));
const MissionsModal = lazy(() => import("./MissionsModal").then(m => ({ default: m.MissionsModal })));
const WorkspaceManagerModal = lazy(() => import("./WorkspaceManagerModal").then(m => ({ default: m.WorkspaceManagerModal })));
const SystemMonitorModal = lazy(() => import("./SystemMonitorModal").then(m => ({ default: m.SystemMonitorModal })));

export interface ModalLayerProps {
  modals: Record<string, boolean>;
  openModal: (name: string) => void;
  closeModal: (name: string) => void;
  appState: any;
  activeSessionId: string | null;
  activeProject: any;
  onCommand: (cmd: string, args?: string) => Promise<void>;
  onNewSession: () => void;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onSend: (text: string, images?: any[]) => Promise<void>;
  onModelSelected: (modelId: string, providerId: string) => void;
  onAgentChange: (agentId: string) => Promise<void>;
  onContextSubmit: (text: string) => Promise<void>;
  onThemeApplied: (colors: any) => void;
  onRestoreWorkspace: (ws: any) => void;
  buildCurrentSnapshot: () => any;
  onCompactContext: () => void;
  onAutonomyChange: (mode: string) => void;
  onDismissSignal: (id: string) => void;
  onQuickSetupDailyCopilot: () => void;
  onQuickSetupDailySummary: () => void;
  onQuickUpgradeAutonomy: () => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  selectedModel: any;
  selectedAgent: string;
  themeMode: any;
  setThemeMode: (m: any) => void;
  fileEditCount: number;
  allPermissions: any[];
  allQuestions: any[];
  sidebarOpen: boolean;
  terminalOpen: boolean;
  neovimOpen: boolean;
  gitOpen: boolean;
  liveActivityEvents: any[];
  watcherStatus: any;
  assistantSignals: any[];
  autonomyMode: any;
  missionCache: any[];
  routineCache: any[];
  delegatedWorkCache: any[];
  activeMemoryItems: any[];
  workspaceCache: any[];
  resumeBriefing: any;
  latestDailySummary: string | null;
  activeWorkspaceName: string | null;
  personalMemoryForInbox: any[];
  splitViewSecondaryId: string | null;
  setSplitViewSecondaryId: (id: string | null) => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  onOpenSkillsUpload?: () => void;
}

const L = ({ children }: { children: React.ReactNode }) => (
  <Suspense fallback={null}>{children}</Suspense>
);

export const ModalLayer: React.FC<ModalLayerProps> = React.memo(function ModalLayer(p) {
  const { modals: m, openModal: o, closeModal: c } = p;

  /** Close `from` modal then open `to` modal */
  const nav = useCallback((from: string, to: string) => () => { c(from); o(to); }, [c, o]);
  /** Adapter: (projectIndex, sessionId) → onSelectSession */
  const selByProj = useCallback(
    (pi: number, sid: string) => p.onSelectSession(sid, pi), [p.onSelectSession],
  );
  /** Navigate to session within active project */
  const navSess = useCallback(
    (sid: string) => p.onSelectSession(sid, p.appState?.active_project ?? 0),
    [p.onSelectSession, p.appState],
  );
  const cl = (k: string) => () => c(k);

  return (
    <>
      {m.commandPalette && (
        <L>
        <CommandPalette
          onClose={cl("commandPalette")} onCommand={p.onCommand}
          onNewSession={p.onNewSession} onToggleSidebar={p.toggleSidebar}
          onToggleTerminal={p.toggleTerminal}
          onOpenModelPicker={() => { c("commandPalette"); o("modelPicker"); }}
          onOpenCheatsheet={() => o("cheatsheet")} onOpenTodoPanel={() => o("todoPanel")}
          onOpenSessionSelector={() => o("sessionSelector")}
          onOpenContextInput={() => o("contextInput")} onOpenSettings={() => o("settings")}
          onOpenWatcher={() => o("watcher")} onOpenContextWindow={() => o("contextWindow")}
          onOpenDiffReview={() => o("diffReview")} onOpenSearch={() => o("searchBar")}
          onOpenCrossSearch={() => o("crossSearch")} onOpenSplitView={() => o("splitView")}
          onOpenSessionGraph={() => o("sessionGraph")}
          onOpenSessionDashboard={() => o("sessionDashboard")}
          onOpenActivityFeed={() => o("activityFeed")}
          onOpenNotificationPrefs={() => o("notificationPrefs")}
          onOpenAssistantCenter={() => o("assistantCenter")}
          onOpenInbox={() => o("inbox")} onOpenMemory={() => o("memory")}
          onOpenAutonomy={() => o("autonomy")} onOpenRoutines={() => o("routines")}
          onOpenDelegation={() => o("delegation")} onOpenMissions={() => o("missions")}
          onOpenWorkspaceManager={() => o("workspaceManager")}
          onOpenSystemMonitor={() => o("systemMonitor")}
          onOpenSkillsUpload={p.onOpenSkillsUpload}
          sessionId={p.activeSessionId}
        />
        </L>
      )}

      {m.modelPicker && (
        <L><ModelPickerModal onClose={cl("modelPicker")} sessionId={p.activeSessionId} onModelSelected={p.onModelSelected} /></L>
      )}
      {m.agentPicker && (
        <L><AgentPickerModal onClose={cl("agentPicker")} currentAgent={p.selectedAgent} onAgentSelected={p.onAgentChange} /></L>
      )}
      {m.themeSelector && (
        <L><ThemeSelectorModal onClose={cl("themeSelector")} onThemeApplied={p.onThemeApplied} themeMode={p.themeMode} onThemeModeChange={p.setThemeMode} /></L>
      )}
      {m.cheatsheet && <L><CheatsheetModal onClose={cl("cheatsheet")} /></L>}
      {m.todoPanel && p.activeSessionId && (
        <L><TodoPanelModal onClose={cl("todoPanel")} sessionId={p.activeSessionId} /></L>
      )}
      {m.sessionSelector && p.appState && (
        <L><SessionSelectorModal onClose={cl("sessionSelector")} projects={p.appState.projects} activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession} /></L>
      )}
      {m.contextInput && (
        <L><ContextInputModal onClose={cl("contextInput")} onSubmit={p.onContextSubmit} /></L>
      )}

      {m.settings && (
        <L>
        <SettingsModal
          onClose={cl("settings")}
          onOpenThemeSelector={nav("settings", "themeSelector")}
          onOpenCheatsheet={nav("settings", "cheatsheet")}
          onOpenNotificationPrefs={nav("settings", "notificationPrefs")}
          onOpenAssistantCenter={nav("settings", "assistantCenter")}
          onOpenMemory={nav("settings", "memory")}
          onOpenAutonomy={nav("settings", "autonomy")}
          onOpenRoutines={nav("settings", "routines")}
          onOpenDelegation={nav("settings", "delegation")}
          onOpenWorkspaceManager={nav("settings", "workspaceManager")}
          onOpenInbox={nav("settings", "inbox")}
          onOpenMissions={nav("settings", "missions")}
          sidebarOpen={p.sidebarOpen} terminalOpen={p.terminalOpen}
          neovimOpen={p.neovimOpen} gitOpen={p.gitOpen}
          onToggleSidebar={p.toggleSidebar} onToggleTerminal={p.toggleTerminal}
          onToggleNeovim={p.toggleNeovim} onToggleGit={p.toggleGit}
        />
        </L>
      )}

      {m.watcher && <L><WatcherModal onClose={cl("watcher")} activeSessionId={p.activeSessionId} /></L>}
      {m.contextWindow && (
        <L><ContextWindowPanel onClose={cl("contextWindow")} sessionId={p.activeSessionId} onCompact={p.onCompactContext} /></L>
      )}
      {m.diffReview && (
        <L><DiffReviewPanel onClose={cl("diffReview")} sessionId={p.activeSessionId} fileEditCount={p.fileEditCount} /></L>
      )}
      {m.crossSearch && p.appState && (
        <L><CrossSessionSearchModal onClose={cl("crossSearch")} projectIdx={p.appState.active_project} onNavigate={navSess} /></L>
      )}
      {m.splitView && p.appState && p.activeSessionId && (
        <L><SplitView
          primarySessionId={p.activeSessionId} secondarySessionId={p.splitViewSecondaryId}
          onChangeSecondary={p.setSplitViewSecondaryId} onClose={cl("splitView")}
          sessions={p.activeProject?.sessions ?? []} appState={p.appState}
          selectedModel={p.selectedModel?.modelID} projectIndex={p.appState.active_project}
        /></L>
      )}

      {m.sessionGraph && <L><SessionGraph onSelectSession={selByProj} onClose={cl("sessionGraph")} activeSessionId={p.activeSessionId} /></L>}
      {m.sessionDashboard && <L><SessionDashboard onSelectSession={selByProj} onClose={cl("sessionDashboard")} activeSessionId={p.activeSessionId} /></L>}
      {m.activityFeed && <L><ActivityFeed sessionId={p.activeSessionId} onClose={cl("activityFeed")} liveEvents={p.liveActivityEvents} /></L>}
      {m.notificationPrefs && <L><NotificationPrefsModal onClose={cl("notificationPrefs")} /></L>}

      {m.assistantCenter && (
        <L>
          <AssistantCenterModal
            onClose={cl("assistantCenter")} autonomyMode={p.autonomyMode}
            assistantSignals={p.assistantSignals} permissions={p.allPermissions}
            questions={p.allQuestions}
            resumeBriefing={p.resumeBriefing} latestDailySummary={p.latestDailySummary}
            onQuickSetupDailyCopilot={p.onQuickSetupDailyCopilot}
            onQuickSetupDailySummary={p.onQuickSetupDailySummary}
            onQuickUpgradeAutonomy={p.onQuickUpgradeAutonomy}
            onOpenInbox={nav("assistantCenter", "inbox")}
            onOpenMissions={nav("assistantCenter", "missions")}
            onOpenMemory={nav("assistantCenter", "memory")}
            onOpenAutonomy={nav("assistantCenter", "autonomy")}
            onOpenRoutines={nav("assistantCenter", "routines")}
            onOpenDelegation={nav("assistantCenter", "delegation")}
            onOpenWorkspaces={nav("assistantCenter", "workspaceManager")}
          />
        </L>
      )}

      {m.inbox && (
        <L>
          <InboxModal
            onClose={cl("inbox")} permissions={p.allPermissions} questions={p.allQuestions}
            watcherStatus={p.watcherStatus} signals={p.assistantSignals}
            activityEvents={p.liveActivityEvents} onDismissSignal={p.onDismissSignal}
            onOpenMissions={nav("inbox", "missions")} onOpenActivityFeed={nav("inbox", "activityFeed")}
            onOpenWatcher={nav("inbox", "watcher")} onPermissionResolved={p.clearPermission}
            onQuestionResolved={p.clearQuestion} activeMemoryItems={p.personalMemoryForInbox}
          />
        </L>
      )}

      {m.missions && (
        <L>
          <MissionsModal
            onClose={cl("missions")} projects={p.appState.projects}
            activeProjectIndex={p.appState.active_project} activeSessionId={p.activeSessionId}
            permissions={p.allPermissions} questions={p.allQuestions}
            activityEvents={p.liveActivityEvents} onOpenInbox={nav("missions", "inbox")}
            activeMemoryItems={p.activeMemoryItems}
          />
        </L>
      )}

      {m.memory && (
        <L><MemoryModal onClose={cl("memory")} projects={p.appState.projects} activeProjectIndex={p.appState.active_project} activeSessionId={p.activeSessionId} /></L>
      )}
      {m.autonomy && (
        <L><AutonomyModal onClose={cl("autonomy")} mode={p.autonomyMode} onChange={p.onAutonomyChange} /></L>
      )}
      {m.routines && (
        <L><RoutinesModal onClose={cl("routines")} missions={p.missionCache} activeSessionId={p.activeSessionId} autonomyMode={p.autonomyMode} appState={p.appState} /></L>
      )}
      {m.delegation && (
        <L><DelegationBoardModal onClose={cl("delegation")} missions={p.missionCache} activeSessionId={p.activeSessionId} onOpenSession={navSess} /></L>
      )}
      {m.workspaceManager && (
        <L><WorkspaceManagerModal onClose={cl("workspaceManager")} onRestore={p.onRestoreWorkspace} onSaveCurrent={p.buildCurrentSnapshot} activeWorkspaceName={p.activeWorkspaceName} /></L>
      )}

      {m.addProject && <L><AddProjectModal onClose={cl("addProject")} /></L>}

      {m.systemMonitor && (
        <L><SystemMonitorModal onClose={cl("systemMonitor")} /></L>
      )}
    </>
  );
});
