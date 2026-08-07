import type { BindingSpec } from "../types";

/**
 * Directional focus for the whole shell, and the sidebar's list keys.
 *
 * Two rules shape this file.
 *
 * `!textInput` is on every entry. A modifier chord normally survives the
 * matcher's insert guard — that is what lets Ctrl+S save from inside a
 * composer — but `Ctrl+H` is Backspace to readline and to every shell running
 * in the terminal widget, and `Ctrl+←` is word-left in every text field on the
 * platform. Focus movement is never worth breaking either, so these opt back
 * out of the guard by naming the condition explicitly. `Ctrl+K Ctrl+←` still
 * works from inside a text field and is the way out of one.
 *
 * `Ctrl+K` itself is bound only on macOS. Everywhere else `mod` is Control, so
 * `ctrl+k` is the prefix of `mod+k mod+t`, `mod+k d` and a dozen more: the
 * matcher waits for a second step rather than firing, and a binding that
 * silently does nothing is worse than one that is absent. `Ctrl+↑` covers the
 * direction on those platforms, and vim mode keeps `Ctrl+W k`.
 */

const NOT_TYPING = "!textInput";

export const BASE_NAV: readonly BindingSpec[] = [
  { key: "ctrl+h", command: "nav.focusLeft", when: NOT_TYPING },
  { key: "ctrl+j", command: "nav.focusDown", when: NOT_TYPING },
  { key: "ctrl+l", command: "nav.focusRight", when: NOT_TYPING },
  { key: "ctrl+k", command: "nav.focusUp", when: NOT_TYPING, platform: "mac" },

  { key: "ctrl+left", command: "nav.focusLeft", when: NOT_TYPING },
  { key: "ctrl+down", command: "nav.focusDown", when: NOT_TYPING },
  { key: "ctrl+up", command: "nav.focusUp", when: NOT_TYPING },
  { key: "ctrl+right", command: "nav.focusRight", when: NOT_TYPING },
];

/**
 * Bare keys in the sidebar.
 *
 * Safe unmodified for the same reason the explorer's are: `focus==sidebar` is
 * only true while the session list owns the keyboard, and a bare key with a
 * `when` clause never survives the insert guard — so the sidebar's own filter
 * field keeps every letter typed into it.
 */
export const BASE_SIDEBAR: readonly BindingSpec[] = [
  { key: "j", command: "sidebar.moveDown", when: "focus==sidebar" },
  { key: "k", command: "sidebar.moveUp", when: "focus==sidebar" },
  { key: "l", command: "sidebar.expand", when: "focus==sidebar" },
  { key: "h", command: "sidebar.collapse", when: "focus==sidebar" },
  { key: "down", command: "sidebar.moveDown", when: "focus==sidebar" },
  { key: "up", command: "sidebar.moveUp", when: "focus==sidebar" },
  { key: "right", command: "sidebar.expand", when: "focus==sidebar" },
  { key: "left", command: "sidebar.collapse", when: "focus==sidebar" },
  { key: "enter", command: "sidebar.open", when: "focus==sidebar" },
];
