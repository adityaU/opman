export interface CommandPaletteProps {
  onClose: () => void;
  onCommand: (command: string, args?: string) => void;
  onNewSession: () => void;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onOpenModelPicker: () => void;
  onOpenCheatsheet: () => void;
  onOpenTodoPanel: () => void;
  onOpenSessionSelector: () => void;
  onOpenContextInput: () => void;
  onOpenSettings: () => void;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
  onOpenDiffReview: () => void;
  onOpenSearch: () => void;
  onOpenCrossSearch: () => void;
  onOpenSplitView?: () => void;
  onOpenSessionGraph?: () => void;
  onOpenSessionDashboard?: () => void;
  onOpenActivityFeed?: () => void;
  onOpenNotificationPrefs?: () => void;
  onOpenInbox?: () => void;
  onOpenAssistantCenter?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
  onOpenDelegation?: () => void;
  onOpenWorkspaceManager?: () => void;
  onOpenMissions?: () => void;
  onOpenSystemMonitor?: () => void;
  sessionId: string | null;
}

export interface PaletteItem {
  id: string;
  category: string;
  label: string;
  description?: string;
  shortcut?: string;
  handler: () => void;
}

export interface PaletteGroup {
  category: string;
  items: PaletteItem[];
}
