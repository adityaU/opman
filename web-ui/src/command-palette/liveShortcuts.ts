import { displayChord } from "../keybindings/chord";
import type { Keymap } from "../keybindings/matcher";
import type { Platform } from "../keybindings/types";
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
  cheatsheet: "system.keybindings",
  "session-selector": "session.switch",
  settings: "system.settings",
  "skills-upload": "system.uploadSkills",
  watcher: "session.watcher",
  "context-window": "chat.contextWindow",
  "diff-review": "git.diffReview",
  search: "chat.find",
  "cross-search": "palette.searchAll",
  "split-view": "layout.toggleSplitView",
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
 * Replace each item's shortcut with its live chord.
 *
 * An item whose command is unbound loses its shortcut entirely, which is the
 * honest rendering: the row still runs, it just has no key.
 */
export function withLiveShortcuts(
  items: readonly PaletteItem[],
  keymap: Keymap,
  platform: Platform,
): PaletteItem[] {
  return items.map((item) => {
    const command = PALETTE_TO_COMMAND[item.id];
    if (!command) return item;
    const [binding] = keymap.chordsFor(command);
    return { ...item, shortcut: binding ? displayChord(binding.seq, platform) : undefined };
  });
}
