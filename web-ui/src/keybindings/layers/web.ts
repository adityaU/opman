import type { BindingSpec } from "../types";

/**
 * The web override layer.
 *
 * Every entry here exists because a browser or macOS takes the canonical chord
 * before the page sees it. Nothing else belongs in this file, and the desktop
 * target has no equivalent — that is what makes adding the desktop build a
 * no-op rather than a second keymap.
 *
 * `Ctrl+Alt` / `Option+Cmd` is the escape space and is reserved for exactly
 * this purpose, so it stays free as more chords get stolen.
 */

/** Taken by every browser on every platform. */
const UNIVERSAL: readonly BindingSpec[] = [
  // Cmd/Ctrl+N and Cmd/Ctrl+Shift+N open browser windows.
  { key: "mod+alt+n", command: "session.new" },
  { key: "mod+alt+shift+n", command: "session.newInProject" },

  // Cmd/Ctrl+W closes the tab, Cmd/Ctrl+Shift+W the window.
  { key: "mod+k w", command: "editor.close", when: "editorOpen" },
  { key: "mod+k c", command: "session.close", when: "sessionActive" },

  // Cmd/Ctrl+1..9 switch browser tabs.
  { key: "mod+alt+1", command: "layout.focusChat" },
  { key: "mod+alt+2", command: "layout.focusEditor" },
  { key: "mod+alt+3", command: "layout.focusTerminal" },
  { key: "mod+alt+4", command: "layout.focusGit" },
  { key: "mod+alt+5", command: "layout.focusBoard" },
  { key: "mod+alt+0", command: "layout.focusExplorer" },

  // Cmd/Ctrl+T opens a browser tab even as the second step of a pending chord,
  // which costs VSCode's Ctrl+K Ctrl+T for the theme picker.
  { key: "mod+k mod+.", command: "system.themeSelector" },
];

/**
 * Firefox devtools and window shortcuts. Scoped to the browser, so Chrome and
 * Safari keep the chord VSCode users expect.
 *
 * A binding here supersedes the base chord for the same command, so a
 * replacement is all that is needed. `palette.commands` is the exception: it
 * holds two chords and only one of them is lost, so that one is removed by name
 * rather than superseded.
 */
const FIREFOX: readonly BindingSpec[] = [
  { key: "mod+shift+p", command: "-palette.commands" },
  { key: "mod+alt+e", command: "layout.toggleEditor" },
  { key: "mod+alt+k", command: "layout.toggleBoard" },
  { key: "mod+alt+o", command: "palette.symbols" },
  { key: "mod+alt+s", command: "session.switch" },
].map((spec) => ({ ...spec, browser: "firefox" as const }));

/** Chords macOS itself consumes, whichever browser is running. */
const MACOS: readonly BindingSpec[] = [
  { key: "mod+alt+,", command: "system.settings" },
].map((spec) => ({ ...spec, platform: "mac" as const }));

/**
 * Ctrl+N opens a browser window on Windows and Linux, which costs vim's
 * `Ctrl+\ Ctrl+N`. macOS is unaffected — its browser chords use Command.
 */
const VIM_TERMINAL_ESCAPE: readonly BindingSpec[] = (["win", "linux"] as const).map(
  (platform) => ({
    key: "ctrl+\\ ctrl+o",
    command: "terminal.copyMode",
    when: "focus==terminal",
    mode: "vim" as const,
    group: "terminal",
    label: "copy mode",
    platform,
  }),
);

export const WEB_LAYER: readonly BindingSpec[] = [
  ...UNIVERSAL,
  ...FIREFOX,
  ...MACOS,
  ...VIM_TERMINAL_ESCAPE,
].map((spec) => ({ ...spec, target: "web" as const }));
