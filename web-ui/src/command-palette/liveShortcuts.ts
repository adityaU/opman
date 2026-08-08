import type { Keymap } from "../keybindings/matcher";
import type { Host, Mode } from "../keybindings/types";
import { chordLabel } from "../keybindings/useChord";
import type { PaletteItem } from "./types";

/**
 * Palette rows show the chord that is actually bound right now.
 *
 * The palette predates the command registry and carries its own item ids and
 * hand-written shortcut strings. Those strings are the app's most visible claim
 * about the keymap, and the moment a user rebinds anything they become a lie —
 * so the label is replaced from the live keymap here rather than maintained by
 * hand in two places.
 */
const PALETTE_TO_COMMAND: Readonly<Record<string, string>> = {
  "new-session": "session.new",
  "model-picker": "engine.model",
  "toggle-sidebar": "layout.toggleSidebar",
  "toggle-terminal": "layout.toggleTerminal",
  keybindings: "system.keybindings",
  "session-selector": "session.switch",
  settings: "system.settings",
  theme: "system.themeSelector",
  "mcp-servers": "system.mcpServers",
  skills: "system.skills",
  watcher: "session.watcher",
  "context-window": "chat.contextWindow",
  "diff-review": "git.diffReview",
  search: "chat.find",
  "cross-search": "palette.searchAll",
  "notification-prefs": "assistant.notifications",
  routines: "assistant.routines",
  autonomy: "assistant.autonomy",
  "personal-memory": "assistant.instructions",
  "system-monitor": "system.monitor",
  refresh: "system.refreshApp",
  "todo-panel": "chat.todoPanel",
  "context-input": "chat.sendContext",
  compact: "chat.compact",
  undo: "chat.undoTurn",
  redo: "chat.redoTurn",
  fork: "session.fork",
  share: "session.share",
};

/**
 * Give each item the chord that is bound to it right now.
 *
 * An item whose command is unbound has no shortcut, which is the honest
 * rendering: the row still runs, it just has no key.
 */
export function withLiveShortcuts(
  items: readonly PaletteItem[],
  keymap: Keymap,
  host: Host,
  mode: Mode,
): PaletteItem[] {
  return items.map((item) => ({
    ...item,
    shortcut: chordLabel(keymap, host, mode, PALETTE_TO_COMMAND[item.id]),
  }));
}
