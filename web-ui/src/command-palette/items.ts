import { PaletteItem, CommandPaletteProps } from "./types";

/**
 * Build the full palette item list from component props.
 * Extracted to keep the main component file under 300 lines.
 *
 * No row carries a chord. `withLiveShortcuts` fills them in from the composed
 * keymap, which is the only thing that knows the platform, the browser
 * overrides, the user's own bindings and which mode they are in — a literal
 * here was a second, slowly-diverging claim about the same keys.
 *
 * A row that *navigates* — every settings section — does not call `onClose`.
 * `history.back()` is asynchronous, so closing this way and then pushing a URL let the
 * queued pop land on the page just pushed, bouncing the user back to their session.
 * `onOpenSettings` closes the palette itself, without touching history.
 */
export function buildPaletteItems(props: CommandPaletteProps): PaletteItem[] {
  const {
    onClose,
    onCommand,
    onNewSession,
    onToggleSidebar,
    onToggleTerminal,
    onOpenModelPicker,
    onOpenTodoPanel,
    onOpenSessionSelector,
    onOpenContextInput,
    onOpenSettings,
    onOpenWatcher,
    onOpenContextWindow,
    onOpenDiffReview,
    onOpenSearch,
    onOpenCrossSearch,
    onOpenNotificationPrefs,
    onOpenMemory,
    onOpenAutonomy,
    onOpenRoutines,
    onOpenSystemMonitor,
    sessionId,
  } = props;

  const items: PaletteItem[] = [
    {
      id: "new-session",
      category: "Sessions",
      label: "New Session",
      handler: () => { onClose(); onNewSession(); },
    },
    {
      id: "model-picker",
      category: "Core",
      label: "Choose Model",
      handler: () => { onOpenModelPicker(); },
    },
    {
      id: "toggle-sidebar",
      category: "Layout",
      label: "Toggle Sidebar",
      handler: () => { onClose(); onToggleSidebar(); },
    },
    {
      id: "toggle-terminal",
      category: "Layout",
      label: "Toggle Terminal",
      handler: () => { onClose(); onToggleTerminal(); },
    },
    {
      id: "keybindings",
      category: "Settings",
      label: "Keyboard Shortcuts",
      description: "See and rebind every shortcut",
      handler: () => onOpenSettings("keybindings"),
    },
    {
      id: "session-selector",
      category: "Sessions",
      label: "Select Session",
      description: "Search across all projects",
      handler: () => { onClose(); onOpenSessionSelector(); },
    },
    {
      id: "settings",
      category: "Settings",
      label: "Settings",
      description: "Appearance, keybindings, MCP servers, skills",
      handler: () => onOpenSettings(),
    },
    {
      id: "theme",
      category: "Settings",
      label: "Color Theme",
      description: "Palette, light or dark, glassy or flat",
      handler: () => onOpenSettings("appearance"),
    },
    {
      id: "mcp-servers",
      category: "Settings",
      label: "MCP Servers",
      description: "Tools every runner can reach",
      handler: () => onOpenSettings("mcp"),
    },
    {
      id: "skills",
      category: "Settings",
      label: "Skills",
      description: "Write, import and edit reusable instructions",
      handler: () => onOpenSettings("skills"),
    },
    {
      id: "watcher",
      category: "Sessions",
      label: "Session Watcher",
      description: "Monitor and auto-continue sessions",
      handler: () => { onClose(); onOpenWatcher(); },
    },
    {
      id: "context-window",
      category: "Analysis",
      label: "Context Window",
      description: "View token usage breakdown",
      handler: () => { onClose(); onOpenContextWindow(); },
    },
    {
      id: "diff-review",
      category: "Analysis",
      label: "Diff Review",
      description: "Review file changes made by AI",
      handler: () => { onClose(); onOpenDiffReview(); },
    },
    {
      id: "search",
      category: "Search",
      label: "Search in Conversation",
      description: "Find text in the current session",
      handler: () => { onClose(); onOpenSearch(); },
    },
    {
      id: "cross-search",
      category: "Search",
      label: "Search All Sessions",
      description: "Search across all sessions in project",
      handler: () => { onClose(); onOpenCrossSearch(); },
    },
    {
      id: "notification-prefs",
      category: "Assistant",
      label: "Notification Preferences",
      description: "Configure session alerts",
      handler: () => { onClose(); onOpenNotificationPrefs?.(); },
    },
    {
      id: "routines",
      category: "Assistant",
      label: "Routines",
      description: "Manage recurring assistant workflows",
      handler: () => { onClose(); onOpenRoutines?.(); },
    },
    {
      id: "autonomy",
      category: "Assistant",
      label: "Autonomy",
      description: "Choose proactive assistant mode",
      handler: () => { onClose(); onOpenAutonomy?.(); },
    },
    {
      id: "personal-memory",
      category: "Assistant",
      label: "Session Instructions",
      description: "Standing guidance sent when a session opens",
      handler: () => { onClose(); onOpenMemory?.(); },
    },
    {
      id: "system-monitor",
      category: "System",
      label: "System Monitor",
      description: "htop-like system resource monitor",
      handler: () => { onClose(); onOpenSystemMonitor?.(); },
    },
    {
      id: "refresh",
      category: "System",
      label: "Refresh Page",
      description: "Reload the application",
      handler: () => { onClose(); window.location.reload(); },
    },
  ];

  // Session-specific items
  if (sessionId) {
    items.push(
      {
        id: "todo-panel",
        category: "Sessions",
        label: "Todo Panel",
        description: "View session todos",
        handler: () => { onClose(); onOpenTodoPanel(); },
      },
      {
        id: "context-input",
        category: "Sessions",
        label: "Send Context",
        description: "Send context to the AI session",
        handler: () => { onClose(); onOpenContextInput(); },
      },
      {
        id: "compact",
        category: "Sessions",
        label: "Compact History",
        description: "Compact conversation to reduce tokens",
        handler: () => { onClose(); onCommand("compact"); },
      },
      {
        id: "undo",
        category: "Sessions",
        label: "Undo",
        description: "Undo last action",
        handler: () => { onClose(); onCommand("undo"); },
      },
      {
        id: "redo",
        category: "Sessions",
        label: "Redo",
        description: "Redo last action",
        handler: () => { onClose(); onCommand("redo"); },
      },
      {
        id: "fork",
        category: "Sessions",
        label: "Fork Session",
        description: "Create a copy of this session",
        handler: () => { onClose(); onCommand("fork"); },
      },
      {
        id: "share",
        category: "Sessions",
        label: "Share Session",
        description: "Get a shareable link",
        handler: () => { onClose(); onCommand("share"); },
      },
    );
  }

  return items;
}
