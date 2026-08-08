import type { CommandDef, CommandId, SlashSpec } from "../types";
import { LAYOUT_COMMANDS, PALETTE_COMMANDS, PROJECT_COMMANDS, SYSTEM_COMMANDS } from "./core";
import { CHAT_COMMANDS } from "./chat";
import { ASSISTANT_COMMANDS, ENGINE_COMMANDS } from "./engine";
import {
  EDITOR_COMMANDS,
  EXPLORER_COMMANDS,
  LSP_COMMANDS,
  RICH_FILE_COMMANDS,
} from "./editor";
import { NAV_COMMANDS, SIDEBAR_COMMANDS } from "./nav";
import { BOARD_COMMANDS, GIT_COMMANDS, TERMINAL_COMMANDS } from "./panels";
import { SESSION_COMMANDS } from "./session";
import { WORKSPACE_COMMANDS, WORKSPACE_OVERLAY_COMMANDS } from "./workspace";

/**
 * The command registry.
 *
 * Nothing in the app should handle a key directly: a surface registers a
 * handler for a command id, and the keymap decides which chord reaches it.
 */
export const COMMANDS: readonly CommandDef[] = [
  ...PALETTE_COMMANDS,
  ...LAYOUT_COMMANDS,
  ...NAV_COMMANDS,
  ...WORKSPACE_COMMANDS,
  ...WORKSPACE_OVERLAY_COMMANDS,
  ...SIDEBAR_COMMANDS,
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

/** A command paired with the slash it answers to — the `slash?` narrowed away. */
export interface SlashCommandDef extends CommandDef {
  readonly slash: SlashSpec;
}

function hasSlash(command: CommandDef): command is SlashCommandDef {
  return command.slash !== undefined;
}

/**
 * opman's own slash commands.
 *
 * These, and only these, are the ones the client both lists and executes. Everything else
 * offered in the composer comes from the runner, which is the only thing that knows what it
 * can be asked to do — opman keeps no catalog of agent commands.
 */
export const OPMAN_SLASH_COMMANDS: readonly SlashCommandDef[] = COMMANDS.filter(
  (command): command is SlashCommandDef => hasSlash(command) && command.slash.where === "opman",
);

const BY_SLASH: ReadonlyMap<string, SlashCommandDef> = new Map(
  OPMAN_SLASH_COMMANDS.map((command) => [command.slash.name, command]),
);

/** The opman command `/name` runs, or `undefined` when the name belongs to the runner. */
export function findSlashCommand(name: string): SlashCommandDef | undefined {
  return BY_SLASH.get(name);
}

/** Commands whose implementation is "forward this slash to whichever agent is serving". */
export const RUNNER_SLASH_COMMANDS: readonly SlashCommandDef[] = COMMANDS.filter(
  (command): command is SlashCommandDef => hasSlash(command) && command.slash.where === "runner",
);

export function commandLabel(command: CommandDef): string {
  return command.label ?? command.title.toLowerCase();
}

/** Category order used by the palette and the cheatsheet. */
export const CATEGORY_ORDER: readonly string[] = [
  "Palette",
  "Navigation",
  "Sessions",
  "Sidebar",
  "Chat",
  "Engine",
  "Workspace",
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
