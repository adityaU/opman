import type { KeyBinding } from "./hooks/useKeyboard";

interface KeyboardDeps {
  openModal: (name: string) => void;
  closeTopModal: () => boolean;
  toggleSidebar: () => void;
  toggleTerminal: () => void;
  toggleNeovim: () => void;
  toggleGit: () => void;
  handleNewSession: () => void;
  toggleSplitView: () => void;
}

export function buildKeyboardShortcuts(deps: KeyboardDeps): KeyBinding[] {
  const {
    openModal,
    closeTopModal,
    toggleSidebar,
    toggleTerminal,
    toggleNeovim,
    toggleGit,
    handleNewSession,
    toggleSplitView,
  } = deps;

  return [
    // Meta+Shift combos
    { key: "p", meta: true, shift: true, handler: () => openModal("commandPalette"), description: "Command Palette" },
    { key: "n", meta: true, shift: true, handler: handleNewSession, description: "New Session" },
    { key: "e", meta: true, shift: true, handler: toggleNeovim, description: "Toggle Editor" },
    { key: "g", meta: true, shift: true, handler: toggleGit, description: "Toggle Git" },
    { key: "t", meta: true, shift: true, handler: () => openModal("todoPanel"), description: "Todo Panel" },
    { key: "s", meta: true, shift: true, handler: () => openModal("sessionSelector"), description: "Session Selector" },
    { key: "k", meta: true, shift: true, handler: () => openModal("contextInput"), description: "Context Input" },
    { key: "w", meta: true, shift: true, handler: () => openModal("watcher"), description: "Session Watcher" },
    { key: "c", meta: true, shift: true, handler: () => openModal("contextWindow"), description: "Context Window" },
    { key: "d", meta: true, shift: true, handler: () => openModal("diffReview"), description: "Diff Review" },
    { key: "f", meta: true, shift: true, handler: () => openModal("crossSearch"), description: "Search All Sessions" },
    { key: "h", meta: true, shift: true, handler: () => openModal("sessionGraph"), description: "Session Graph" },
    { key: "o", meta: true, shift: true, handler: () => openModal("sessionDashboard"), description: "Session Dashboard" },
    { key: "a", meta: true, shift: true, handler: () => openModal("activityFeed"), description: "Activity Feed" },
    { key: "i", meta: true, shift: true, handler: () => openModal("notificationPrefs"), description: "Notification Preferences" },
    { key: ".", meta: true, shift: true, handler: () => openModal("assistantCenter"), description: "Assistant Center" },
    { key: "u", meta: true, shift: true, handler: () => openModal("inbox"), description: "Assistant Inbox" },
    { key: "b", meta: true, shift: true, handler: () => openModal("delegation"), description: "Delegation Board" },
    { key: "r", meta: true, shift: true, handler: () => openModal("routines"), description: "Routines" },
    { key: "j", meta: true, shift: true, handler: () => openModal("autonomy"), description: "Autonomy" },
    { key: "y", meta: true, shift: true, handler: () => openModal("memory"), description: "Personal Memory" },
    { key: "m", meta: true, shift: true, handler: () => openModal("missions"), description: "Missions" },
    { key: "l", meta: true, shift: true, handler: () => openModal("workspaceManager"), description: "Workspace Manager" },

    // Meta-only combos
    { key: "'", meta: true, handler: () => openModal("modelPicker"), description: "Model Picker" },
    { key: "b", meta: true, handler: toggleSidebar, description: "Toggle Sidebar" },
    { key: "`", meta: true, handler: toggleTerminal, description: "Toggle Terminal" },
    { key: ",", meta: true, handler: () => openModal("settings"), description: "Settings" },
    { key: "f", meta: true, handler: () => openModal("searchBar"), description: "Search in Conversation" },
    { key: "\\", meta: true, handler: toggleSplitView, description: "Toggle Split View" },

    // Shift-only
    { key: "?", shift: true, handler: () => openModal("cheatsheet"), description: "Keybinding Cheatsheet" },

    // Escape
    { key: "Escape", handler: () => { closeTopModal(); }, description: "Close Modal" },
  ];
}
