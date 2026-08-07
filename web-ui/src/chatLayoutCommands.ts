import type { CommandHandler } from "./keybindings/KeymapContext";
import type { ModalName } from "./hooks/useModalState";

/**
 * The global command handlers.
 *
 * Replaces `buildKeyboardShortcuts`, which paired a chord with a callback. Here
 * a callback is paired with a *command id* and the keymap decides the chord —
 * which is what makes every one of these rebindable from the keybindings view
 * without touching this file.
 */

export interface CommandDeps {
  readonly openModal: (name: ModalName) => void;
  readonly toggleModal: (name: ModalName) => void;
  readonly closeTopModal: () => boolean;
  readonly toggleSidebar: () => void;
  readonly toggleTerminal: () => void;
  readonly toggleEditor: () => void;
  readonly toggleGit: () => void;
  readonly toggleBoard: () => void;
  readonly toggleDebug: () => void;
  readonly newSession: () => void;
  readonly runSlashCommand: (name: string) => void;
  readonly openMemoryActive: () => void;
  readonly reloadApp: () => void;
}

/** Commands whose whole implementation is "open this modal". */
const MODAL_COMMANDS: Readonly<Record<string, ModalName>> = {
  "palette.commands": "commandPalette",
  "palette.searchAll": "crossSearch",
  "session.switch": "sessionSelector",
  "session.watcher": "watcher",
  "chat.find": "searchBar",
  "chat.sendContext": "contextInput",
  "chat.contextWindow": "contextWindow",
  "chat.todoPanel": "todoPanel",
  "engine.palette": "modelPicker",
  "engine.model": "modelPicker",
  "engine.agent": "agentPicker",
  "git.diffReview": "diffReview",
  "assistant.routines": "routines",
  "assistant.autonomy": "autonomy",
  "assistant.notifications": "notificationPrefs",
  "assistant.autoOpen": "autoOpen",
  "system.settings": "settings",
  "system.keybindings": "cheatsheet",
  "system.themeSelector": "themeSelector",
  "system.monitor": "systemMonitor",
  "system.processHealth": "processHealth",
  "project.add": "addProject",
};

/** Commands that dispatch an existing slash command. */
const SLASH_COMMANDS: Readonly<Record<string, string>> = {
  "chat.compact": "compact",
  "chat.undoTurn": "undo",
  "chat.redoTurn": "redo",
  "chat.clear": "clear",
  "chat.abort": "cancel",
  "chat.copyTranscript": "copy",
  "session.fork": "fork",
  "session.share": "share",
};

export function buildCommandHandlers(deps: CommandDeps): Record<string, CommandHandler> {
  const handlers: Record<string, CommandHandler> = {
    "layout.toggleSidebar": deps.toggleSidebar,
    "layout.toggleTerminal": deps.toggleTerminal,
    "layout.toggleEditor": deps.toggleEditor,
    "layout.toggleGit": deps.toggleGit,
    "layout.toggleBoard": deps.toggleBoard,
    "layout.toggleSplitView": () => deps.toggleModal("splitView"),
    // Escape is a command like any other, so the chord that dismisses a modal
    // is configurable rather than hard-coded into the listener.
    "layout.escape": () => {
      deps.closeTopModal();
    },
    "session.new": deps.newSession,
    "session.newInProject": deps.newSession,
    "assistant.instructions": deps.openMemoryActive,
    "system.debugPanel": deps.toggleDebug,
    "system.refreshApp": deps.reloadApp,
  };

  for (const [command, modal] of Object.entries(MODAL_COMMANDS)) {
    handlers[command] = () => deps.openModal(modal);
  }

  for (const [command, slash] of Object.entries(SLASH_COMMANDS)) {
    handlers[command] = () => deps.runSlashCommand(slash);
  }

  return handlers;
}
