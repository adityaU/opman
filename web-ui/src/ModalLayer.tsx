import React, { Suspense, lazy, useCallback } from "react";
import { openSettings } from "./settings-page/useSettingsRoute";
import type { SettingsSection } from "./settings-page/useSettingsRoute";

// Lazy-load all modals — they are conditionally rendered and rarely all open at once
const CommandPalette = lazy(() => import("./CommandPalette").then(m => ({ default: m.CommandPalette })));
const ModelPickerModal = lazy(() => import("./ModelPickerModal").then(m => ({ default: m.ModelPickerModal })));
const AgentPickerModal = lazy(() => import("./AgentPickerModal").then(m => ({ default: m.AgentPickerModal })));
const TodoPanelModal = lazy(() => import("./TodoPanelModal").then(m => ({ default: m.TodoPanelModal })));
const SessionSelectorModal = lazy(() => import("./SessionSelectorModal").then(m => ({ default: m.SessionSelectorModal })));
const ContextInputModal = lazy(() => import("./ContextInputModal").then(m => ({ default: m.ContextInputModal })));
const WatcherModal = lazy(() => import("./WatcherModal").then(m => ({ default: m.WatcherModal })));
const AddProjectModal = lazy(() => import("./AddProjectModal").then(m => ({ default: m.AddProjectModal })));
const ContextWindowPanel = lazy(() => import("./ContextWindowPanel").then(m => ({ default: m.ContextWindowPanel })));
const DiffReviewPanel = lazy(() => import("./DiffReviewPanel").then(m => ({ default: m.DiffReviewPanel })));
const CrossSessionSearchModal = lazy(() => import("./CrossSessionSearchModal").then(m => ({ default: m.CrossSessionSearchModal })));

const NotificationPrefsModal = lazy(() => import("./NotificationPrefsModal").then(m => ({ default: m.NotificationPrefsModal })));
const AutonomyModal = lazy(() => import("./AutonomyModal").then(m => ({ default: m.AutonomyModal })));
const MemoryModal = lazy(() => import("./MemoryModal").then(m => ({ default: m.MemoryModal })));
const RoutinesModal = lazy(() => import("./RoutinesModal").then(m => ({ default: m.RoutinesModal })));
const SystemMonitorModal = lazy(() => import("./SystemMonitorModal").then(m => ({ default: m.SystemMonitorModal })));
const ProcessHealthDrawer = lazy(() => import("./ProcessHealthDrawer").then(m => ({ default: m.ProcessHealthDrawer })));
const AutoOpenModal = lazy(() => import("./AutoOpenModal").then(m => ({ default: m.AutoOpenModal })));

export interface ModalLayerProps {
  modals: Record<string, boolean>;
  openModal: (name: string) => void;
  closeModal: (name: string) => void;
  closeModalSilent: (name: string) => void;
  appState: any;
  activeSessionId: string | null;
  currentRunner: string;
  activeProject: any;
  /** URL-derived active project index — sole source of truth. */
  activeProjectIndex: number;
  onCommand: (cmd: string, args?: string) => Promise<void>;
  onNewSession: () => void;
  onSelectSession: (sessionId: string, projectIdx: number) => void;
  onSend: (text: string, images?: any[]) => Promise<boolean>;
  onModelSelected: (modelId: string, providerId: string) => void;
  onAgentChange: (agentId: string) => Promise<void>;
  onContextSubmit: (text: string) => Promise<void>;
  onCompactContext: () => void;
  onAutonomyChange: (mode: string) => void;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  selectedModel: any;
  selectedAgent: string;
  fileEditCount: number;
  allPermissions: any[];
  allQuestions: any[];
  watcherStatus: any;
  autonomyMode: any;
  routineCache: any[];
  activeMemoryItems: any[];
  memoryFilterActive: boolean;
  /** Open memory modal showing all memories (sets filterActive=false). */
  openMemoryAll: () => void;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
}

const L = ({ children }: { children: React.ReactNode }) => (
  <Suspense fallback={null}>{children}</Suspense>
);

export const ModalLayer: React.FC<ModalLayerProps> = React.memo(function ModalLayer(p) {
  const { modals: m, openModal: o, closeModal: c, closeModalSilent: cs } = p;

  /** Navigate to session within active project */
  const navSess = useCallback(
    (sid: string) => p.onSelectSession(sid, p.activeProjectIndex),
    [p.onSelectSession, p.appState],
  );
  const cl = (k: string) => () => c(k);
  /** Leave the palette and go to a settings section.
   *
   *  Closes *silently*: the ordinary close pops the history entry the palette pushed, and
   *  `history.back()` being asynchronous meant that pop landed on the settings URL this
   *  then pushed — which is what sent the user back to their session instead. */
  const navSettings = useCallback((section?: SettingsSection) => {
    cs("commandPalette");
    openSettings(section);
  }, [cs]);

  return (
    <>
      {m.commandPalette && (
        <L>
        <CommandPalette
          onClose={cl("commandPalette")} onCommand={p.onCommand}
          onNewSession={p.onNewSession} onToggleSidebar={p.toggleSidebar}
          onToggleTerminal={p.toggleTerminal}
          onOpenModelPicker={() => { c("commandPalette"); o("modelPicker"); }}
          onOpenTodoPanel={() => o("todoPanel")}
          onOpenSessionSelector={() => o("sessionSelector")}
          onOpenContextInput={() => o("contextInput")} onOpenSettings={navSettings}
          onOpenWatcher={() => o("watcher")} onOpenContextWindow={() => o("contextWindow")}
          onOpenDiffReview={() => o("diffReview")} onOpenSearch={() => o("searchBar")}
          onOpenCrossSearch={() => o("crossSearch")}
          onOpenNotificationPrefs={() => o("notificationPrefs")}
          onOpenMemory={p.openMemoryAll}
          onOpenAutonomy={() => o("autonomy")} onOpenRoutines={() => o("routines")}
          onOpenSystemMonitor={() => o("systemMonitor")}
          sessionId={p.activeSessionId}
        />
        </L>
      )}

      {m.modelPicker && (
        <L><ModelPickerModal onClose={cl("modelPicker")} onCloseSilent={() => cs("modelPicker")} sessionId={p.activeSessionId} currentRunner={p.currentRunner} onModelSelected={p.onModelSelected} /></L>
      )}
      {m.agentPicker && (
        <L><AgentPickerModal onClose={cl("agentPicker")} currentAgent={p.selectedAgent} currentRunner={p.currentRunner} onAgentSelected={p.onAgentChange} /></L>
      )}
      {m.todoPanel && p.activeSessionId && (
        <L><TodoPanelModal onClose={cl("todoPanel")} sessionId={p.activeSessionId} /></L>
      )}
      {m.sessionSelector && p.appState && (
        <L><SessionSelectorModal onClose={cl("sessionSelector")} projects={p.appState.projects} activeSessionId={p.activeSessionId} onSelectSession={p.onSelectSession} /></L>
      )}
      {m.contextInput && (
        <L><ContextInputModal onClose={cl("contextInput")} onSubmit={p.onContextSubmit} /></L>
      )}

      {m.watcher && <L><WatcherModal onClose={cl("watcher")} activeSessionId={p.activeSessionId} /></L>}
      {m.contextWindow && (
        <L><ContextWindowPanel onClose={cl("contextWindow")} sessionId={p.activeSessionId} onCompact={p.onCompactContext} /></L>
      )}
      {m.diffReview && (
        <L><DiffReviewPanel onClose={cl("diffReview")} sessionId={p.activeSessionId} fileEditCount={p.fileEditCount} /></L>
      )}
      {m.crossSearch && p.appState && (
        <L><CrossSessionSearchModal onClose={cl("crossSearch")}          projectIdx={p.activeProjectIndex} onNavigate={navSess} /></L>
      )}
      {m.notificationPrefs && <L><NotificationPrefsModal onClose={cl("notificationPrefs")} /></L>}

      {m.memory && (
        <L><MemoryModal onClose={cl("memory")} projects={p.appState.projects} activeProjectIndex={p.activeProjectIndex} activeSessionId={p.activeSessionId} filterActive={p.memoryFilterActive} /></L>
      )}
      {m.autonomy && (
        <L><AutonomyModal onClose={cl("autonomy")} mode={p.autonomyMode} onChange={p.onAutonomyChange} /></L>
      )}
      {m.routines && (
        <L><RoutinesModal onClose={cl("routines")} activeSessionId={p.activeSessionId} autonomyMode={p.autonomyMode} appState={p.appState} /></L>
      )}

      {m.addProject && <L><AddProjectModal onClose={cl("addProject")} /></L>}

      {m.systemMonitor && (
        <L><SystemMonitorModal onClose={cl("systemMonitor")} /></L>
      )}

      {m.processHealth && (
        <L><ProcessHealthDrawer onClose={cl("processHealth")} /></L>
      )}

      {m.autoOpen && (
        <L><AutoOpenModal onClose={cl("autoOpen")} /></L>
      )}
    </>
  );
});
