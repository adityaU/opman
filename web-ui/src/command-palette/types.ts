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
  onOpenNotificationPrefs?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
  onOpenSystemMonitor?: () => void;
  onOpenSkillsUpload?: () => void;
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
