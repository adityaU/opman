import type { CommandHandler } from "./keybindings/KeymapContext";
import { RUNNER_SLASH_COMMANDS } from "./keybindings/commands";
import type { ModalName } from "./hooks/useModalState";
import { toggleSettings } from "./settings-page/useSettingsRoute";
import type { SettingsSection } from "./settings-page/useSettingsRoute";

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
  readonly newSession: () => void;
  readonly abortSession: () => void;
  readonly copyTranscript: () => void;
  /** Send a slash command to the agent serving the session. */
  readonly forwardToRunner: (name: string) => void;
  readonly openMemoryActive: () => void;
  readonly openMemoryAll: () => void;
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
  "system.monitor": "systemMonitor",
  "system.processHealth": "processHealth",
  "project.add": "addProject",
};

/**
 * Commands whose whole implementation is "go to a settings section".
 *
 * Settings is a page, so these navigate rather than opening anything — which is also what
 * makes each section linkable and survivable across a reload.
 */
const SETTINGS_COMMANDS: Readonly<Record<string, SettingsSection>> = {
  "system.settings": "appearance",
  "system.themeSelector": "appearance",
  "system.keybindings": "keybindings",
  "system.mcpServers": "mcp",
  "system.skills": "skills",
};

export function buildCommandHandlers(deps: CommandDeps): Record<string, CommandHandler> {
  const handlers: Record<string, CommandHandler> = {
    "layout.toggleSidebar": deps.toggleSidebar,
    "layout.toggleTerminal": deps.toggleTerminal,
    "layout.toggleEditor": deps.toggleEditor,
    "layout.toggleGit": deps.toggleGit,
    "layout.toggleBoard": deps.toggleBoard,
    // Escape is a command like any other, so the chord that dismisses a modal
    // is configurable rather than hard-coded into the listener.
    "layout.escape": () => {
      deps.closeTopModal();
    },
    "session.new": deps.newSession,
    "session.newInProject": deps.newSession,
    "chat.abort": deps.abortSession,
    "chat.copyTranscript": deps.copyTranscript,
    "assistant.instructions": deps.openMemoryActive,
    "assistant.memories": deps.openMemoryAll,
    "system.refreshApp": deps.reloadApp,
  };

  for (const [command, modal] of Object.entries(MODAL_COMMANDS)) {
    handlers[command] = () => deps.openModal(modal);
  }

  for (const [command, section] of Object.entries(SETTINGS_COMMANDS)) {
    handlers[command] = () => toggleSettings(section);
  }

  // Commands the *agent* runs. The registry names them, so a chord for "compact" and the
  // `/compact` the runner advertises are the same command reaching the same place — and
  // adding one is a line in the registry rather than a second table to keep in step.
  for (const command of RUNNER_SLASH_COMMANDS) {
    handlers[command.id] = () => deps.forwardToRunner(command.slash.name);
  }

  return handlers;
}
