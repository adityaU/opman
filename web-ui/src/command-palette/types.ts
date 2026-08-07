import type { SettingsSection } from "../settings-page/useSettingsRoute";

export interface CommandPaletteProps {
  onClose: () => void;
  onCommand: (command: string, args?: string) => void;
  onNewSession: () => void;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onOpenModelPicker: () => void;
  onOpenTodoPanel: () => void;
  onOpenSessionSelector: () => void;
  onOpenContextInput: () => void;
  /** Open the settings page, optionally on a named section. */
  onOpenSettings: (section?: SettingsSection) => void;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
  onOpenDiffReview: () => void;
  onOpenSearch: () => void;
  onOpenCrossSearch: () => void;
  onOpenNotificationPrefs?: () => void;
  onOpenMemory?: () => void;
  onOpenAutonomy?: () => void;
  onOpenRoutines?: () => void;
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
