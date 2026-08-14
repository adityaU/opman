import type { BindingSpec } from "../types";

/** Terminal, git and board. */

export const BASE_TERMINAL: readonly BindingSpec[] = [
  { key: "ctrl+shift+`", command: "terminal.newShell" },
  { key: "mod+k `", command: "terminal.selectShell" },
  { key: "alt+]", command: "terminal.nextShell", when: "focus==terminal" },
  { key: "alt+[", command: "terminal.previousShell", when: "focus==terminal" },
  { key: "mod+k x", command: "terminal.killShell", when: "focus==terminal" },
  { key: "f2", command: "terminal.renameShell", when: "focus==terminal" },
  { key: "mod+f", command: "terminal.find", when: "focus==terminal" },
  { key: "enter", command: "terminal.findNext", when: "terminalFindOpen" },
  { key: "shift+enter", command: "terminal.findPrevious", when: "terminalFindOpen" },
  { key: "mod+c", command: "terminal.copy", when: "focus==terminal" },
  { key: "mod+v", command: "terminal.paste", when: "focus==terminal" },
  { key: "ctrl+escape", command: "terminal.leaveFocus", when: "focus==terminal" },
];

export const BASE_GIT: readonly BindingSpec[] = [
  { key: "alt+1", command: "git.changesTab", when: "focus==git" },
  { key: "alt+2", command: "git.logTab", when: "focus==git" },
  { key: "f5", command: "git.refresh", when: "focus==git" },
  // Normal mode only: in vim mode Space is the leader, and staging is `s`.
  { key: "space", command: "git.toggleStageFile", when: "focus==git", mode: "normal" },
  { key: "enter", command: "git.openDiff", when: "focus==git" },
  { key: "down", command: "git.nextFile", when: "focus==git", mode: "normal" },
  { key: "up", command: "git.previousFile", when: "focus==git", mode: "normal" },
  { key: "alt+down", command: "git.nextHunk", when: "focus==git" },
  { key: "alt+up", command: "git.previousHunk", when: "focus==git" },
  { key: "mod+k mod+]", command: "git.expandAll", when: "focus==git" },
  { key: "mod+k mod+[", command: "git.collapseAll", when: "focus==git" },
  { key: "mod+k mod+g", command: "git.focusCommitMessage", when: "gitRepo" },
  { key: "mod+enter", command: "git.commit", when: "gitStaged" },
  { key: "mod+k mod+;", command: "git.generateCommitMessage", when: "gitStaged" },
  { key: "mod+k b", command: "git.switchBranch", when: "gitRepo" },
  { key: "mod+k mod+'", command: "git.createPr", when: "gitRepo" },
  { key: "mod+shift+d", command: "git.diffReview", when: "sessionActive" },
  { key: "alt+left", command: "git.back", when: "focus==git" },
];

export const BASE_BOARD: readonly BindingSpec[] = [
  { key: "down", command: "board.moveDown", when: "focus==board" },
  { key: "up", command: "board.moveUp", when: "focus==board" },
  { key: "left", command: "board.moveLeft", when: "focus==board" },
  { key: "right", command: "board.moveRight", when: "focus==board" },
  { key: "enter", command: "board.openTask", when: "focus==board" },
  { key: "mod+alt+n", command: "board.newTask", when: "focus==board" },
  { key: "f2", command: "board.editTask", when: "taskSelected" },
  { key: "mod+enter", command: "board.launch", when: "taskSelected" },
  { key: "mod+alt+enter", command: "board.openTaskSession", when: "taskHasSession" },
  { key: "alt+left", command: "board.moveTaskLeft", when: "taskSelected" },
  { key: "alt+right", command: "board.moveTaskRight", when: "taskSelected" },
  { key: "f5", command: "board.refresh", when: "focus==board" },
];
