import type { BindingSpec } from "../types";
import { VIM_LEADER } from "./vim-leader";

/**
 * Bare keys and bracket motions.
 *
 * Every bare key carries a `when` naming the surface that owns it, which is
 * what lets `a` create a file in the explorer and answer a permission prompt in
 * the chat without the two ever meeting. Insert mode is handled by the matcher,
 * not here: while a text input holds focus only modifier chords are dispatched.
 */

function vim(specs: readonly Omit<BindingSpec, "mode">[], group: string): BindingSpec[] {
  return specs.map((spec) => ({ ...spec, mode: "vim" as const, group }));
}

/** `]x` / `[x` — the same shape as vim's own next/previous family. */
const MOTIONS = vim(
  [
    { key: "]s", command: "session.next", label: "next session" },
    { key: "[s", command: "session.previous", label: "previous session" },
    { key: "]b", command: "editor.nextFile", label: "next file", when: "editorOpen" },
    { key: "[b", command: "editor.previousFile", label: "previous file", when: "editorOpen" },
    { key: "]t", command: "terminal.nextShell", label: "next terminal", when: "terminalOpen" },
    { key: "[t", command: "terminal.previousShell", label: "previous terminal", when: "terminalOpen" },
    { key: "]m", command: "chat.nextMessage", label: "next message", when: "focus==chat" },
    { key: "[m", command: "chat.previousMessage", label: "previous message", when: "focus==chat" },
    { key: "]d", command: "lsp.nextDiagnostic", label: "next problem", when: "editorOpen" },
    { key: "[d", command: "lsp.previousDiagnostic", label: "previous problem", when: "editorOpen" },
    { key: "]h", command: "git.nextHunk", label: "next hunk", when: "focus==git" },
    { key: "[h", command: "git.previousHunk", label: "previous hunk", when: "focus==git" },
    { key: "]p", command: "viewer.nextPage", label: "next page", when: "focus==viewer" },
    { key: "[p", command: "viewer.previousPage", label: "previous page", when: "focus==viewer" },
  ],
  "motion",
);

const CHAT = vim(
  [
    { key: "i", command: "chat.focusComposer", when: "focus==chat" },
    { key: "a", command: "chat.focusComposer", when: "focus==chat" },
    { key: "u", command: "chat.undoTurn", when: "focus==chat" },
    { key: "ctrl+r", command: "chat.redoTurn", when: "focus==chat" },
    { key: "/", command: "chat.find", when: "focus==chat" },
    { key: "n", command: "chat.findNext", when: "findOpen" },
    { key: "N", command: "chat.findPrevious", when: "findOpen" },
    { key: "ctrl+d", command: "chat.scrollDown", when: "focus==chat" },
    { key: "ctrl+u", command: "chat.scrollUp", when: "focus==chat" },
    { key: "gg", command: "chat.scrollTop", when: "focus==chat", label: "first message" },
    { key: "G", command: "chat.scrollBottom", when: "focus==chat" },
    { key: "za", command: "chat.toggleToolCall", when: "focus==chat", label: "toggle tool call" },
    { key: "zR", command: "chat.expandAll", when: "focus==chat", label: "expand all" },
    { key: "zM", command: "chat.collapseAll", when: "focus==chat", label: "collapse all" },
  ],
  "chat",
);

const EXPLORER = vim(
  [
    // `hjkl` moved to the base layer — the explorer answers to them in normal
    // mode too, and a second copy here would only differ by `mode`.
    { key: "o", command: "explorer.open", when: "focus==explorer" },
    { key: "a", command: "explorer.newFile", when: "focus==explorer" },
    { key: "A", command: "explorer.newFolder", when: "focus==explorer" },
    { key: "r", command: "explorer.rename", when: "focus==explorer" },
    { key: "d", command: "explorer.delete", when: "focus==explorer" },
    { key: "m", command: "explorer.contextMenu", when: "focus==explorer" },
    { key: "R", command: "explorer.reload", when: "focus==explorer" },
    { key: "U", command: "explorer.upload", when: "focus==explorer" },
    { key: "y", command: "explorer.copyPath", when: "focus==explorer" },
    { key: "zM", command: "explorer.collapseAll", when: "focus==explorer", label: "collapse all" },
  ],
  "explorer",
);

const GIT = vim(
  [
    { key: "j", command: "git.nextFile", when: "focus==git" },
    { key: "k", command: "git.previousFile", when: "focus==git" },
    { key: "s", command: "git.stageFile", when: "focus==git" },
    { key: "u", command: "git.unstageFile", when: "focus==git" },
    { key: "X", command: "git.discard", when: "focus==git" },
    { key: "zR", command: "git.expandAll", when: "focus==git", label: "expand all" },
    { key: "zM", command: "git.collapseAll", when: "focus==git", label: "collapse all" },
    { key: "ctrl+o", command: "git.back", when: "focus==git" },
  ],
  "git",
);

const BOARD = vim(
  [
    { key: "j", command: "board.moveDown", when: "focus==board" },
    { key: "k", command: "board.moveUp", when: "focus==board" },
    { key: "h", command: "board.moveLeft", when: "focus==board" },
    { key: "l", command: "board.moveRight", when: "focus==board" },
    { key: "n", command: "board.newTask", when: "focus==board" },
    { key: "e", command: "board.editTask", when: "taskSelected" },
    { key: "x", command: "board.launch", when: "taskSelected" },
    { key: "o", command: "board.openTaskSession", when: "taskHasSession" },
    { key: "<lt>", command: "board.moveTaskLeft", when: "taskSelected" },
    { key: "<gt>", command: "board.moveTaskRight", when: "taskSelected" },
    { key: "A", command: "board.archiveTask", when: "taskSelected" },
  ],
  "board",
);

const TERMINAL = vim(
  [{ key: "ctrl+\\ ctrl+n", command: "terminal.copyMode", when: "focus==terminal", label: "copy mode" }],
  "terminal",
);

/**
 * `gt` / `gT` — a workspace window is a vim tab, so it gets vim's tab motion.
 * `<leader>w]` and `[` do the same thing for anyone who found the namespace
 * first.
 */
const WINDOW_MOTIONS = vim(
  [
    { key: "gt", command: "workspace.nextWindow", label: "next window" },
    { key: "gT", command: "workspace.previousWindow", label: "previous window" },
  ],
  "motion",
);

/**
 * Bare `Ctrl-w`, aliasing the `<leader>w` namespace onto the prefix vim users
 * reach for first.
 *
 * macOS only, and not by preference: on Windows and Linux `Ctrl+W` closes the
 * browser tab before the page sees the keydown, and `host.ts` lists it as
 * reserved, so binding it there would fail the conflict test — correctly, since
 * the binding could never fire. Those platforms keep `<leader>w`.
 */
const WINDOW_PREFIX = vim(
  [
    { key: "ctrl+w v", command: "workspace.splitRight", label: "split right" },
    { key: "ctrl+w s", command: "workspace.splitDown", label: "split down" },
    { key: "ctrl+w h", command: "workspace.focusLeft", label: "left" },
    { key: "ctrl+w j", command: "workspace.focusDown", label: "down" },
    { key: "ctrl+w k", command: "workspace.focusUp", label: "up" },
    { key: "ctrl+w l", command: "workspace.focusRight", label: "right" },
    { key: "ctrl+w H", command: "workspace.movePaneLeft", label: "move left" },
    { key: "ctrl+w J", command: "workspace.movePaneDown", label: "move down" },
    { key: "ctrl+w K", command: "workspace.movePaneUp", label: "move up" },
    { key: "ctrl+w L", command: "workspace.movePaneRight", label: "move right" },
    { key: "ctrl+w w", command: "workspace.cyclePane", label: "cycle" },
    { key: "ctrl+w c", command: "workspace.closePane", label: "close" },
    { key: "ctrl+w o", command: "workspace.closeOtherPanes", label: "only" },
    { key: "ctrl+w z", command: "workspace.zoomPane", label: "zoom" },
    { key: "ctrl+w =", command: "workspace.equalize", label: "equalize" },
    { key: "ctrl+w T", command: "workspace.movePaneToNewWindow", label: "to new window" },
  ],
  "window",
).map((spec) => ({ ...spec, platform: "mac" as const }));

/**
 * `hjkl` while the target overlay is up. Vim only — normal mode keeps bare
 * letters free and uses the arrows the base layer binds.
 *
 * `h` moving the highlight is why the horizontal split gets `s` here: vim
 * already calls that `:split`, so the key a vim user reaches for is the one
 * they know rather than the one the base layer needed.
 */
const TARGET = vim(
  [
    // Vim keeps `h` for movement, so the base layer's horizontal split is
    // withdrawn here and re-offered as `s` — vim's own name for it.
    { key: "h", command: "-workspace.targetSplitDown", when: "workspaceTargeting" },
    { key: "h", command: "workspace.focusLeft", when: "workspaceTargeting" },
    { key: "j", command: "workspace.focusDown", when: "workspaceTargeting" },
    { key: "k", command: "workspace.focusUp", when: "workspaceTargeting" },
    { key: "l", command: "workspace.focusRight", when: "workspaceTargeting" },
    { key: "s", command: "workspace.targetSplitDown", when: "workspaceTargeting" },
  ],
  "window",
);

export const VIM_LAYER: readonly BindingSpec[] = [
  ...VIM_LEADER,
  ...MOTIONS,
  ...WINDOW_MOTIONS,
  ...WINDOW_PREFIX,
  ...TARGET,
  ...CHAT,
  ...EXPLORER,
  ...GIT,
  ...BOARD,
  ...TERMINAL,
];
