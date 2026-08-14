import type { BindingSpec } from "../types";

/**
 * The vim leader tree.
 *
 * Two invariants hold across this file and are enforced by the conflict test,
 * not by review: no node is both a prefix and a command — which is why focusing
 * the explorer is `<leader>ee` and not `<leader>e` — and no leaf repeats within
 * a namespace. `group` and `label` are what the which-key hints render.
 */

interface Leaf {
  readonly key: string;
  readonly command: string;
  readonly label: string;
  readonly when?: string;
}

function namespace(prefix: string, group: string, leaves: readonly Leaf[]): BindingSpec[] {
  return leaves.map((leaf) => ({
    key: `<leader>${prefix}${leaf.key}`,
    command: leaf.command,
    when: leaf.when,
    mode: "vim" as const,
    group,
    label: leaf.label,
  }));
}

const FIND = namespace("f", "find", [
  { key: "f", command: "palette.files", label: "file" },
  { key: "s", command: "palette.symbols", label: "symbol", when: "editorOpen" },
  { key: "l", command: "palette.line", label: "line", when: "editorOpen" },
  { key: "r", command: "palette.recentFiles", label: "recent" },
  { key: "d", command: "lsp.diagnosticsList", label: "problems" },
]);

const BUFFERS = namespace("b", "buffers", [
  { key: "b", command: "editor.listOpenFiles", label: "list" },
  { key: "s", command: "editor.save", label: "save", when: "editorDirty" },
  { key: "a", command: "editor.saveAll", label: "save all", when: "anyDirty" },
  { key: "u", command: "editor.revert", label: "revert", when: "editorDirty" },
  { key: "d", command: "editor.close", label: "close", when: "editorOpen" },
]);

const EXPLORER = namespace("e", "explorer", [
  { key: "e", command: "layout.focusExplorer", label: "focus" },
  { key: "f", command: "explorer.newFile", label: "new file" },
  { key: "d", command: "explorer.newFolder", label: "new folder" },
  { key: "r", command: "explorer.reload", label: "reload" },
  { key: "u", command: "explorer.upload", label: "upload" },
  { key: "w", command: "explorer.download", label: "download", when: "focus==explorer" },
]);

const SESSIONS = namespace("s", "sessions", [
  { key: "s", command: "session.switch", label: "switch" },
  { key: "n", command: "session.new", label: "new" },
  { key: "N", command: "session.newInProject", label: "new in project" },
  { key: "r", command: "session.rename", label: "rename", when: "sessionActive" },
  { key: "p", command: "session.togglePin", label: "pin", when: "sessionActive" },
  { key: "c", command: "session.close", label: "close", when: "sessionActive" },
  { key: "d", command: "session.delete", label: "delete", when: "sessionActive" },
  { key: "f", command: "session.fork", label: "fork", when: "sessionActive" },
  { key: "h", command: "session.share", label: "share", when: "sessionActive" },
  { key: "w", command: "session.watcher", label: "watcher" },
  { key: "/", command: "session.filterSidebar", label: "filter" },
]);

const CHAT = namespace("c", "chat", [
  { key: "a", command: "chat.abort", label: "abort", when: "sessionBusy" },
  { key: "/", command: "chat.slashCommands", label: "slash", when: "sessionActive" },
  { key: "p", command: "chat.attachImage", label: "picture", when: "sessionActive" },
  { key: "t", command: "chat.attachTerminal", label: "terminal", when: "sessionActive" },
  { key: "q", command: "chat.queuePanel", label: "queue", when: "hasQueue" },
  { key: "Q", command: "chat.clearQueue", label: "clear queue", when: "hasQueue" },
  { key: "r", command: "chat.retry", label: "retry", when: "sessionActive" },
  { key: "c", command: "chat.compact", label: "compact", when: "sessionActive" },
  { key: "l", command: "chat.clear", label: "clear", when: "sessionActive" },
  { key: "x", command: "chat.sendContext", label: "send context", when: "sessionActive" },
  { key: "w", command: "chat.contextWindow", label: "context window", when: "sessionActive" },
  { key: "d", command: "chat.todoPanel", label: "todos", when: "sessionActive" },
  { key: "s", command: "chat.find", label: "search" },
  { key: "u", command: "chat.usage", label: "usage", when: "sessionActive" },
]);

const ENGINE = namespace("m", "model", [
  { key: "m", command: "engine.palette", label: "engine" },
  { key: "r", command: "engine.runner", label: "runner" },
  { key: "o", command: "engine.model", label: "model" },
  { key: "a", command: "engine.agent", label: "agent" },
  { key: "e", command: "engine.effort", label: "effort", when: "runnerHasEffort" },
  { key: "p", command: "engine.permissionMode", label: "permissions" },
]);

const GIT = namespace("g", "git", [
  { key: "g", command: "layout.toggleGit", label: "panel" },
  { key: "s", command: "git.stageAll", label: "stage all", when: "gitRepo" },
  { key: "u", command: "git.unstageAll", label: "unstage all", when: "gitRepo" },
  { key: "c", command: "git.changesTab", label: "changes", when: "gitRepo" },
  { key: "l", command: "git.logTab", label: "log", when: "gitRepo" },
  { key: "i", command: "git.focusCommitMessage", label: "commit input", when: "gitRepo" },
  { key: "m", command: "git.generateCommitMessage", label: "message", when: "gitStaged" },
  { key: "b", command: "git.switchBranch", label: "branch", when: "gitRepo" },
  { key: "o", command: "git.switchRepo", label: "repo" },
  { key: "p", command: "git.createPr", label: "pull request", when: "gitRepo" },
  { key: "v", command: "git.sendToReview", label: "review", when: "gitRepo" },
  { key: "d", command: "git.diffReview", label: "diff review", when: "sessionActive" },
  { key: "r", command: "git.refresh", label: "refresh", when: "gitRepo" },
  { key: "x", command: "git.discard", label: "discard", when: "focus==git" },
  { key: "y", command: "git.acceptChange", label: "accept", when: "diffReviewOpen" },
  { key: "n", command: "git.revertChange", label: "revert", when: "diffReviewOpen" },
]);

const TERMINAL = namespace("t", "terminal", [
  { key: "n", command: "terminal.newShell", label: "new" },
  { key: "N", command: "terminal.selectShell", label: "switch" },
  { key: "c", command: "terminal.killShell", label: "kill", when: "terminalOpen" },
  { key: "r", command: "terminal.renameShell", label: "rename", when: "terminalOpen" },
  { key: "s", command: "terminal.find", label: "search", when: "terminalOpen" },
  { key: "l", command: "terminal.clear", label: "clear", when: "terminalOpen" },
  { key: "t", command: "terminal.expand", label: "expand", when: "terminalOpen" },
]);

const BOARD = namespace("k", "kanban", [
  { key: "k", command: "layout.toggleBoard", label: "board" },
  { key: "f", command: "board.findTask", label: "find" },
  { key: "n", command: "board.newTask", label: "new task" },
  { key: "l", command: "board.configureLanes", label: "lanes" },
  { key: "r", command: "board.refresh", label: "refresh" },
  { key: "b", command: "board.switchBoard", label: "switch board" },
  { key: "a", command: "board.addNote", label: "note", when: "taskSelected" },
  { key: "x", command: "board.abortRun", label: "abort", when: "taskRunning" },
]);

const ASSISTANT = namespace("a", "assistant", [
  { key: "r", command: "assistant.routines", label: "routines" },
  { key: "s", command: "assistant.instructions", label: "instructions" },
  { key: "u", command: "assistant.autonomy", label: "autonomy" },
  { key: "n", command: "assistant.notifications", label: "notifications" },
  { key: "o", command: "assistant.autoOpen", label: "auto-open" },
]);

const LANGUAGE = namespace("l", "language", [
  { key: "r", command: "lsp.rename", label: "rename", when: "editorOpen" },
  { key: "f", command: "lsp.format", label: "format", when: "editorOpen" },
  { key: "a", command: "lsp.codeAction", label: "action", when: "editorOpen" },
  { key: "d", command: "lsp.goToDefinition", label: "definition", when: "editorOpen" },
  { key: "h", command: "lsp.hover", label: "hover", when: "editorOpen" },
  { key: "p", command: "editor.togglePreview", label: "preview", when: "editorPreviewable" },
  { key: "s", command: "editor.sendToChat", label: "send to chat", when: "editorOpen" },
  { key: "R", command: "sheet.addRow", label: "add row", when: "focus==sheet" },
  { key: "C", command: "sheet.addColumn", label: "add column", when: "focus==sheet" },
]);

/**
 * Panes and windows — vim's own `Ctrl-w` set, moved under the leader.
 *
 * A pane here is a vim window and a window is a vim tab, so `v`/`s` split,
 * `hjkl` move between them, `HJKL` move them, `c` closes, `o` keeps only this
 * one, `z` zooms and `=` equalizes. Anyone who knows `Ctrl-w` knows this
 * namespace without reading it; the same set is aliased onto bare `Ctrl-w`
 * itself in `vim.ts`, where the browser leaves it alone.
 */
const WINDOW = namespace("w", "window", [
  { key: "v", command: "workspace.splitRight", label: "split right" },
  { key: "s", command: "workspace.splitDown", label: "split down" },
  { key: "h", command: "workspace.focusLeft", label: "left" },
  { key: "j", command: "workspace.focusDown", label: "down" },
  { key: "k", command: "workspace.focusUp", label: "up" },
  { key: "l", command: "workspace.focusRight", label: "right" },
  { key: "H", command: "workspace.movePaneLeft", label: "move left" },
  { key: "J", command: "workspace.movePaneDown", label: "move down" },
  { key: "K", command: "workspace.movePaneUp", label: "move up" },
  { key: "L", command: "workspace.movePaneRight", label: "move right" },
  { key: "w", command: "workspace.cyclePane", label: "cycle" },
  { key: "c", command: "workspace.closePane", label: "close" },
  { key: "o", command: "workspace.closeOtherPanes", label: "only" },
  { key: "z", command: "workspace.zoomPane", label: "zoom" },
  { key: "=", command: "workspace.equalize", label: "equalize" },
  { key: "p", command: "workspace.openWidget", label: "pick widget" },
  { key: "m", command: "workspace.paneMenu", label: "menu" },
  { key: "T", command: "workspace.movePaneToNewWindow", label: "to new window" },
  { key: "n", command: "workspace.newWindow", label: "new window" },
  { key: "x", command: "workspace.closeWindow", label: "close window" },
  { key: "]", command: "workspace.nextWindow", label: "next window" },
  { key: "[", command: "workspace.previousWindow", label: "prev window" },
  { key: "g", command: "workspace.windowSwitcher", label: "go to window" },
  { key: "r", command: "workspace.renameWindow", label: "rename window" },
  { key: "b", command: "layout.toggleSidebar", label: "sidebar" },
  { key: "t", command: "workspace.toggleRail", label: "rail" },
  { key: "Z", command: "workspace.toggleZen", label: "zen" },
]);

const YANK = namespace("y", "yank", [
  { key: "y", command: "chat.copyResponse", label: "response", when: "sessionActive" },
  { key: "a", command: "chat.copyTranscript", label: "transcript", when: "sessionActive" },
  { key: "p", command: "explorer.copyPath", label: "path", when: "focus==explorer" },
  { key: "f", command: "editor.copyContents", label: "file", when: "editorOpen" },
  { key: "c", command: "chat.copyCodeBlock", label: "code block", when: "focus==chat" },
]);

const UI = namespace("u", "ui", [
  { key: "t", command: "system.themeSelector", label: "theme" },
  { key: "v", command: "system.toggleVimMode", label: "vim mode" },
  { key: "m", command: "system.monitor", label: "monitor" },
  { key: "h", command: "system.processHealth", label: "health" },
  { key: "r", command: "system.refreshApp", label: "reload" },
  { key: "s", command: "system.mcpServers", label: "mcp servers" },
  { key: "k", command: "system.skills", label: "skills" },
]);

const PROJECT = namespace("p", "project", [
  { key: "a", command: "project.add", label: "add" },
  { key: "r", command: "project.remove", label: "remove" },
  { key: "s", command: "project.switch", label: "switch" },
]);

/** Leaves that hang directly off the leader and are never prefixes. */
const DIRECT: readonly BindingSpec[] = [
  { key: "<leader><leader>", command: "palette.commands", label: "commands" },
  { key: "<leader>/", command: "palette.searchAll", label: "search all" },
  { key: "<leader>,", command: "system.settings", label: "settings" },
  { key: "<leader>?", command: "system.keybindings", label: "help" },
  // The digits address panes, matching the numbers the target overlay paints
  // on them — one counting language for "which pane", everywhere.
  { key: "<leader>1", command: "workspace.focusPane1", label: "pane 1" },
  { key: "<leader>2", command: "workspace.focusPane2", label: "pane 2" },
  { key: "<leader>3", command: "workspace.focusPane3", label: "pane 3" },
  { key: "<leader>4", command: "workspace.focusPane4", label: "pane 4" },
  { key: "<leader>5", command: "workspace.focusPane5", label: "pane 5" },
  { key: "<leader>6", command: "workspace.focusPane6", label: "pane 6" },
  { key: "<leader>7", command: "workspace.focusPane7", label: "pane 7" },
  { key: "<leader>8", command: "workspace.focusPane8", label: "pane 8" },
  { key: "<leader>9", command: "workspace.focusPane9", label: "pane 9" },
].map((spec) => ({ ...spec, mode: "vim" as const, group: "leader" }));

export const VIM_LEADER: readonly BindingSpec[] = [
  ...DIRECT,
  ...FIND,
  ...BUFFERS,
  ...EXPLORER,
  ...SESSIONS,
  ...CHAT,
  ...ENGINE,
  ...GIT,
  ...TERMINAL,
  ...BOARD,
  ...ASSISTANT,
  ...LANGUAGE,
  ...WINDOW,
  ...YANK,
  ...UI,
  ...PROJECT,
];
