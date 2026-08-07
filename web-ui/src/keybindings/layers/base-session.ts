import type { BindingSpec } from "../types";

/**
 * Sessions and the conversation surface.
 *
 * `when` is carried on every binding whose chord is shared across surfaces —
 * mod+f is Find in three different panels, and the scope is what keeps that
 * from being a collision.
 */

export const BASE_SESSION: readonly BindingSpec[] = [
  { key: "mod+n", command: "session.new" },
  { key: "mod+shift+n", command: "session.newInProject" },
  { key: "mod+pagedown", command: "session.next" },
  { key: "mod+pageup", command: "session.previous" },
  { key: "mod+k mod+f", command: "session.filterSidebar" },
  { key: "mod+k r", command: "session.rename", when: "sessionActive" },
  { key: "mod+k p", command: "session.togglePin", when: "sessionActive" },
  { key: "mod+shift+w", command: "session.close", when: "sessionActive" },
  { key: "mod+k delete", command: "session.delete", when: "sessionActive" },
  { key: "mod+k f", command: "session.fork", when: "sessionActive" },
  { key: "mod+k h", command: "session.share", when: "sessionActive" },
  // session.watcher has no chord: it is infrequent, and every second step that
  // would have fit its mnemonic is consumed by macOS.
];

export const BASE_CHAT: readonly BindingSpec[] = [
  { key: "ctrl+i", command: "chat.focusComposer", when: "focus==chat" },
  { key: "enter", command: "chat.send", when: "composerFocused" },
  { key: "@", command: "chat.mention", when: "composerFocused" },
  { key: "shift+enter", command: "chat.newline", when: "composerFocused" },
  { key: "escape", command: "chat.abort", when: "sessionBusy" },
  { key: "mod+k i", command: "chat.attachImage", when: "sessionActive" },
  { key: "mod+k t", command: "chat.attachTerminal", when: "sessionActive" },
  { key: "mod+k q", command: "chat.queuePanel", when: "hasQueue" },
  { key: "mod+alt+z", command: "chat.undoTurn", when: "sessionActive" },
  { key: "mod+alt+shift+z", command: "chat.redoTurn", when: "sessionActive" },
  { key: "mod+k mod+r", command: "chat.retry", when: "sessionActive" },
  { key: "mod+k mod+c", command: "chat.compact", when: "sessionActive" },
  { key: "mod+k mod+l", command: "chat.clear", when: "sessionActive" },
  { key: "mod+k mod+x", command: "chat.sendContext", when: "sessionActive" },
  { key: "mod+k mod+e", command: "chat.contextWindow", when: "sessionActive" },
  { key: "mod+k j", command: "chat.todoPanel", when: "sessionActive" },
  { key: "mod+k y", command: "chat.copyResponse", when: "sessionActive" },
  { key: "mod+k mod+j", command: "chat.copyTranscript", when: "sessionActive" },
  { key: "mod+f", command: "chat.find", when: "focus==chat" },
  { key: "enter", command: "chat.findNext", when: "findOpen" },
  { key: "shift+enter", command: "chat.findPrevious", when: "findOpen" },
  { key: "alt+down", command: "chat.nextMessage", when: "focus==chat" },
  { key: "alt+up", command: "chat.previousMessage", when: "focus==chat" },
  { key: "home", command: "chat.scrollTop", when: "focus==chat" },
  { key: "end", command: "chat.scrollBottom", when: "focus==chat" },
  { key: "mod+k mod+]", command: "chat.expandAll", when: "focus==chat" },
  { key: "mod+k mod+[", command: "chat.collapseAll", when: "focus==chat" },
  { key: "enter", command: "permission.allowOnce", when: "permissionPending" },
  { key: "a", command: "permission.allowAlways", when: "permissionPending" },
  { key: "r", command: "permission.reject", when: "permissionPending" },
  { key: "mod+enter", command: "question.submit", when: "questionPending" },
  { key: "escape", command: "question.dismiss", when: "questionPending" },
];
