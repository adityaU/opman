import type { BindingSpec } from "../types";

/**
 * Workspace bindings, normal mode.
 *
 * Every chord here that VSCode already owns is used unbent — split right is
 * `mod+\`, focus group N is `mod+1..9`, focus/move group in a direction are the
 * `mod+k` arrow chords. The web layer is what escapes the ones a browser takes,
 * so the desktop build inherits this file with an empty override.
 */

const ORDINALS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

const FOCUS_ORDINALS: readonly BindingSpec[] = ORDINALS.map((n) => ({
  key: `mod+${n}`,
  command: `workspace.focusPane${n}`,
}));

export const BASE_WORKSPACE: readonly BindingSpec[] = [
  // Splitting — VSCode's own chords, freed by removing the old split view.
  { key: "mod+\\", command: "workspace.splitRight" },
  { key: "mod+k mod+\\", command: "workspace.splitDown" },

  // Pane lifecycle. `mod+w` is the browser's, and every obvious second step is taken: `w` and
  // `x` by editor.close and terminal.closeTab, `q` by chat.queuePanel, `u` by
  // editor.revert, `r` by session.rename. Those are `when`-scoped so the
  // conflict test allows the overlap — but the matcher picks one, and a chord
  // that works only when no session is active is worse than a free letter.
  { key: "mod+k d", command: "workspace.closePane" },
  { key: "mod+k e", command: "workspace.closeOtherPanes" },
  { key: "mod+k z", command: "workspace.zoomPane" },
  { key: "mod+k =", command: "workspace.equalize" },

  // Focus.
  ...FOCUS_ORDINALS,
  { key: "mod+k mod+left", command: "workspace.focusLeft" },
  { key: "mod+k mod+right", command: "workspace.focusRight" },
  { key: "mod+k mod+up", command: "workspace.focusUp" },
  { key: "mod+k mod+down", command: "workspace.focusDown" },
  { key: "f6", command: "workspace.cyclePane" },

  // Moving a pane — VSCode's "move editor group" chords.
  { key: "mod+k shift+left", command: "workspace.movePaneLeft" },
  { key: "mod+k shift+right", command: "workspace.movePaneRight" },
  { key: "mod+k shift+up", command: "workspace.movePaneUp" },
  { key: "mod+k shift+down", command: "workspace.movePaneDown" },
  { key: "mod+k shift+enter", command: "workspace.movePaneToNewWindow" },

  // Windows.
  { key: "mod+k n", command: "workspace.newWindow" },
  { key: "mod+k shift+w", command: "workspace.closeWindow" },
  { key: "mod+k f2", command: "workspace.renameWindow" },
  { key: "mod+alt+]", command: "workspace.nextWindow" },
  { key: "mod+alt+[", command: "workspace.previousWindow" },
  { key: "mod+k g", command: "workspace.windowSwitcher" },

  // Opening.
  { key: "mod+k o", command: "workspace.openWidget" },
  { key: "mod+k enter", command: "workspace.paneMenu" },

  // Chrome. The sidebar keeps `mod+b` over in base-core.
  { key: "mod+k l", command: "workspace.toggleRail" },
  { key: "mod+k mod+z", command: "workspace.toggleZen" },
  { key: "mod+k h", command: "workspace.togglePaneHeaders" },
  // Zen hides the way back out, so it owns Escape while it is on. Scoped, or
  // it would swallow the Escape that closes everything else.
  { key: "escape", command: "workspace.toggleZen", when: "workspaceZen" },
];

/**
 * Bare keys while the pane-target overlay is up.
 *
 * Unmodified letters and digits are safe here only because the `when` clause
 * scopes them to an overlay that owns the keyboard for as long as it is
 * visible — the same trick `focus==git` uses for `j`/`k` in the git panel.
 */
export const BASE_WORKSPACE_TARGET: readonly BindingSpec[] = [
  ...ORDINALS.map((n) => ({
    key: String(n),
    command: `workspace.targetPane${n}`,
    when: "workspaceTargeting",
  })),
  { key: "enter", command: "workspace.targetAccept", when: "workspaceTargeting" },
  { key: "escape", command: "workspace.targetCancel", when: "workspaceTargeting" },
  { key: "s", command: "workspace.targetSplitDown", when: "workspaceTargeting" },
  { key: "v", command: "workspace.targetSplitRight", when: "workspaceTargeting" },
  { key: "n", command: "workspace.targetNewWindow", when: "workspaceTargeting" },
  // Arrows move the highlight; the letters are vim's and are added by the vim
  // layer so normal mode keeps `h`/`l` free for typing into a focused pane.
  { key: "left", command: "workspace.focusLeft", when: "workspaceTargeting" },
  { key: "right", command: "workspace.focusRight", when: "workspaceTargeting" },
  { key: "up", command: "workspace.focusUp", when: "workspaceTargeting" },
  { key: "down", command: "workspace.focusDown", when: "workspaceTargeting" },
];
