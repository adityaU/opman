import type { CommandDef } from "../types";

/**
 * Palette, layout and system commands.
 *
 * Every action in the app is registered as a command exactly once. The palette,
 * the cheatsheet, the which-key hints and the keybindings view all read this
 * registry, so an action cannot exist without being reachable.
 */

export const PALETTE_COMMANDS: readonly CommandDef[] = [
  { id: "palette.commands", title: "Show All Commands", category: "Palette", label: "commands", slash: { name: "commands", where: "opman" } },
  { id: "palette.files", title: "Go to File…", category: "Palette", label: "file" },
  { id: "palette.symbols", title: "Go to Symbol in File…", category: "Palette", when: "editorOpen", label: "symbol" },
  { id: "palette.searchAll", title: "Search All Sessions…", category: "Palette", label: "search all", slash: { name: "cross-search", where: "opman" } },
  { id: "palette.line", title: "Go to Line…", category: "Palette", when: "editorOpen", label: "line" },
  { id: "palette.recentFiles", title: "Open Recent File…", category: "Palette", label: "recent" },
];

/**
 * The palette's other prefixes are not commands of their own — they open a
 * surface that already has one: `s ` is `session.switch`, `m ` is
 * `engine.palette`, `t ` is `system.themeSelector`, `k ` is `board.findTask`
 * and `/` is `chat.slashCommands`.
 */

export const LAYOUT_COMMANDS: readonly CommandDef[] = [
  { id: "layout.toggleSidebar", title: "Toggle Sidebar", category: "Layout", label: "sidebar", slash: { name: "sidebar", where: "opman" } },
  { id: "layout.toggleTerminal", title: "Toggle Terminal Panel", category: "Layout", label: "terminal", slash: { name: "terminal", where: "opman" } },
  { id: "layout.toggleEditor", title: "Toggle Editor and Explorer", category: "Layout", label: "editor", slash: { name: "editor", where: "opman" } },
  { id: "layout.toggleGit", title: "Toggle Git Panel", category: "Layout", label: "git", slash: { name: "git", where: "opman" } },
  { id: "layout.toggleBoard", title: "Toggle Board", category: "Layout", label: "board", slash: { name: "board", where: "opman" } },
  { id: "layout.focusExplorer", title: "Focus Explorer", category: "Layout", label: "explorer" },
  { id: "layout.escape", title: "Dismiss or Return to Chat", category: "Layout" },
];

export const SYSTEM_COMMANDS: readonly CommandDef[] = [
  { id: "system.settings", title: "Open Settings", category: "System", label: "settings", slash: { name: "settings", where: "opman" } },
  { id: "system.keybindings", title: "Settings: Keyboard Shortcuts", category: "System", label: "help", slash: { name: "keys", where: "opman" } },
  { id: "system.themeSelector", title: "Settings: Color Theme", category: "System", label: "theme", slash: { name: "theme", where: "opman" } },
  { id: "system.mcpServers", title: "Settings: MCP Servers", category: "System", label: "mcp", slash: { name: "mcp", where: "opman" } },
  { id: "system.skills", title: "Settings: Skills", category: "System", label: "skills", slash: { name: "skills", where: "opman" } },
  { id: "system.toggleVimMode", title: "Toggle Vim Mode", category: "System", label: "vim mode" },
  { id: "system.monitor", title: "System Monitor", category: "System", label: "monitor", slash: { name: "system", where: "opman" } },
  { id: "system.processHealth", title: "Process Health", category: "System", label: "health", slash: { name: "health", where: "opman" } },
  { id: "system.refreshApp", title: "Reload Application", category: "System", label: "reload" },
];

export const PROJECT_COMMANDS: readonly CommandDef[] = [
  { id: "project.add", title: "Add Project…", category: "Project", label: "add" },
  { id: "project.remove", title: "Remove Project", category: "Project", label: "remove" },
  { id: "project.switch", title: "Switch Project…", category: "Project", label: "switch" },
];
