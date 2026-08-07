import type { BindingSpec } from "../types";

/**
 * The canonical layer: palette, layout and system.
 *
 * Authored with `mod+` (Command on macOS, Control elsewhere) and with the
 * chord VSCode uses, unbent by any browser. The web layer is what moves a
 * binding that a browser would steal, so the desktop build inherits this map
 * unchanged.
 */

export const BASE_PALETTE: readonly BindingSpec[] = [
  { key: "mod+shift+p", command: "palette.commands" },
  { key: "f1", command: "palette.commands" },
  { key: "mod+p", command: "palette.files" },
  { key: "mod+shift+o", command: "palette.symbols" },
  { key: "mod+shift+f", command: "palette.searchAll" },
  { key: "ctrl+g", command: "palette.line" },
  { key: "mod+shift+s", command: "session.switch" },
  // Second steps avoid meta+{t,w,q,m,n,h,space}: macOS consumes those even
  // while a chord is pending, so they are unusable at any position.
  { key: "mod+k mod+t", command: "system.themeSelector" },
  { key: "mod+k mod+k", command: "board.findTask" },
  { key: "ctrl+r", command: "palette.recentFiles" },
];

export const BASE_LAYOUT: readonly BindingSpec[] = [
  { key: "mod+b", command: "layout.toggleSidebar" },
  { key: "ctrl+`", command: "layout.toggleTerminal" },
  { key: "mod+shift+e", command: "layout.toggleEditor" },
  { key: "ctrl+shift+g", command: "layout.toggleGit" },
  { key: "mod+shift+k", command: "layout.toggleBoard" },
  { key: "mod+\\", command: "layout.toggleSplitView" },
  { key: "mod+1", command: "layout.focusChat" },
  { key: "mod+2", command: "layout.focusEditor" },
  { key: "mod+3", command: "layout.focusTerminal" },
  { key: "mod+4", command: "layout.focusGit" },
  { key: "mod+5", command: "layout.focusBoard" },
  { key: "mod+0", command: "layout.focusExplorer" },
  { key: "f6", command: "layout.cycleFocusForward" },
  { key: "shift+f6", command: "layout.cycleFocusBackward" },
  { key: "mod+k z", command: "layout.maximizePanel" },
  { key: "escape", command: "layout.escape" },
];

export const BASE_SYSTEM: readonly BindingSpec[] = [
  { key: "mod+,", command: "system.settings" },
  { key: "mod+k mod+s", command: "system.keybindings" },
  { key: "mod+k mod+v", command: "system.toggleVimMode" },
];

export const BASE_ASSISTANT: readonly BindingSpec[] = [
  { key: "mod+k mod+y", command: "assistant.instructions" },
];

export const BASE_ENGINE: readonly BindingSpec[] = [
  { key: "mod+k m", command: "engine.palette" },
  { key: "mod+k mod+o", command: "engine.runner" },
  { key: "mod+'", command: "engine.model" },
  { key: "mod+k mod+a", command: "engine.agent" },
];
