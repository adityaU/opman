import type { CommandDef, CommandId } from "../types";
import { LAYOUT_COMMANDS, PALETTE_COMMANDS, PROJECT_COMMANDS, SYSTEM_COMMANDS } from "./core";
import { CHAT_COMMANDS } from "./chat";
import { ASSISTANT_COMMANDS, ENGINE_COMMANDS } from "./engine";
import {
  EDITOR_COMMANDS,
  EXPLORER_COMMANDS,
  LSP_COMMANDS,
  RICH_FILE_COMMANDS,
} from "./editor";
import { BOARD_COMMANDS, GIT_COMMANDS, TERMINAL_COMMANDS } from "./panels";
import { SESSION_COMMANDS } from "./session";

/**
 * The command registry.
 *
 * Nothing in the app should handle a key directly: a surface registers a
 * handler for a command id, and the keymap decides which chord reaches it.
 */
export const COMMANDS: readonly CommandDef[] = [
  ...PALETTE_COMMANDS,
  ...LAYOUT_COMMANDS,
  ...SESSION_COMMANDS,
  ...CHAT_COMMANDS,
  ...ENGINE_COMMANDS,
  ...EDITOR_COMMANDS,
  ...LSP_COMMANDS,
  ...RICH_FILE_COMMANDS,
  ...EXPLORER_COMMANDS,
  ...TERMINAL_COMMANDS,
  ...GIT_COMMANDS,
  ...BOARD_COMMANDS,
  ...ASSISTANT_COMMANDS,
  ...SYSTEM_COMMANDS,
  ...PROJECT_COMMANDS,
];

const BY_ID: ReadonlyMap<CommandId, CommandDef> = new Map(COMMANDS.map((c) => [c.id, c]));

export function findCommand(id: CommandId): CommandDef | undefined {
  return BY_ID.get(id);
}

export function commandLabel(command: CommandDef): string {
  return command.label ?? command.title.toLowerCase();
}

/** Category order used by the palette and the cheatsheet. */
export const CATEGORY_ORDER: readonly string[] = [
  "Palette",
  "Sessions",
  "Chat",
  "Engine",
  "Layout",
  "Editor",
  "Language",
  "Explorer",
  "Terminal",
  "Git",
  "Board",
  "Assistant",
  "Document",
  "Spreadsheet",
  "Viewer",
  "Project",
  "System",
];
