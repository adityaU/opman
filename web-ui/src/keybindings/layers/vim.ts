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
    { key: "]t", command: "terminal.nextTab", label: "next terminal", when: "terminalOpen" },
    { key: "[t", command: "terminal.previousTab", label: "previous terminal", when: "terminalOpen" },
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
    { key: "j", command: "explorer.moveDown", when: "focus==explorer" },
    { key: "k", command: "explorer.moveUp", when: "focus==explorer" },
    { key: "l", command: "explorer.expand", when: "focus==explorer" },
    { key: "h", command: "explorer.collapse", when: "focus==explorer" },
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

export const VIM_LAYER: readonly BindingSpec[] = [
  ...VIM_LEADER,
  ...MOTIONS,
  ...CHAT,
  ...EXPLORER,
  ...GIT,
  ...BOARD,
  ...TERMINAL,
];
