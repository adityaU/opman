/**
 * Which terminal shortcut a key press is, decided before xterm sees it.
 *
 * A terminal cannot use the usual copy and paste keys as they are: Ctrl+C is the interrupt
 * signal and Ctrl+Shift+C opens the browser's developer tools. So the keys are settled
 * here, once, and the rules are the ones other terminals use:
 *
 * - Copy: Cmd+C on a Mac; Ctrl+Shift+C or Ctrl+Insert elsewhere; and plain Ctrl+C when text
 *   is selected (nothing is selected while a program is being interrupted).
 * - Paste: Cmd+V, Ctrl+Shift+V and Shift+Insert, left to the browser's own paste event so
 *   that bracketed paste and the permission rules work as they do everywhere else.
 * - Select all: Cmd+A, Ctrl+Shift+A.
 * - Font size: Ctrl/Cmd and `+`, `-`; Ctrl/Cmd+Shift+0 resets.
 * - Scroll: Shift+Home and Shift+End jump to the top and bottom of the scrollback.
 */

export type KeyAction =
  | "copy"
  | "paste"
  | "selectAll"
  | "zoomIn"
  | "zoomOut"
  | "zoomReset"
  | "scrollTop"
  | "scrollBottom";

export interface KeyFacts {
  readonly key: string;
  readonly ctrl: boolean;
  readonly meta: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

export function factsOf(event: KeyboardEvent): KeyFacts {
  return {
    key: event.key,
    ctrl: event.ctrlKey,
    meta: event.metaKey,
    shift: event.shiftKey,
    alt: event.altKey,
  };
}

export function keyAction(k: KeyFacts, mac: boolean, hasSelection: boolean): KeyAction | null {
  const key = k.key.toLowerCase();
  if (k.alt) return null;
  const primary = mac ? k.meta && !k.ctrl : k.ctrl && !k.meta;

  if (mac) {
    if (primary && !k.shift && key === "c" && hasSelection) return "copy";
    if (primary && !k.shift && key === "v") return "paste";
    if (primary && !k.shift && key === "a") return "selectAll";
  } else {
    if (primary && k.shift && key === "c") return "copy";
    if (primary && !k.shift && key === "insert") return "copy";
    if (primary && !k.shift && key === "c" && hasSelection) return "copy";
    if (primary && k.shift && key === "v") return "paste";
    if (!k.ctrl && !k.meta && k.shift && key === "insert") return "paste";
    if (primary && k.shift && key === "a") return "selectAll";
  }

  if (primary && !k.shift && (key === "=" || key === "+")) return "zoomIn";
  if (primary && (key === "+" || key === "=") && k.shift) return "zoomIn";
  if (primary && !k.shift && (key === "-" || key === "_")) return "zoomOut";
  if (primary && k.shift && (key === ")" || key === "0")) return "zoomReset";

  if (!k.ctrl && !k.meta && k.shift && key === "home") return "scrollTop";
  if (!k.ctrl && !k.meta && k.shift && key === "end") return "scrollBottom";
  return null;
}

/** Whether the action is one the browser must still get to finish itself. */
export function leavesToBrowser(action: KeyAction): boolean {
  return action === "paste";
}

/** The shortcut for the menu to show. These are handled by the terminal itself, not the keymap. */
export function shortcutLabel(action: "copy" | "paste" | "selectAll", mac: boolean): string {
  const labels = {
    copy: mac ? "⌘C" : "Ctrl+Shift+C",
    paste: mac ? "⌘V" : "Ctrl+Shift+V",
    selectAll: mac ? "⌘A" : "Ctrl+Shift+A",
  };
  return labels[action];
}

export function isMac(): boolean {
  return /mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent);
}
