import { useMemo } from "react";
import { useCommands, useWhenContext } from "../keybindings/useCommand";
import { useNavTree } from "../keybindings/navRegions";
import type { CommandHandler } from "../keybindings/KeymapContext";
import { withViewTransition } from "./viewTransition";
import type { WorkspaceAction } from "./reducer";
import type { Direction, PaneId, Workspace } from "./types";

/**
 * Every workspace command, wired to the reducer.
 *
 * The surface registers what it can do and publishes what is true about it; the
 * layers decide which chord reaches which command. That separation is why the
 * pane menu, the rail and the opener are all views onto this one list rather
 * than three parallel capabilities — and why "fully keyboard navigable" is a
 * property of the design rather than a feature that has to be maintained.
 */

const ORDINALS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;
const DIRECTIONS: readonly Direction[] = ["left", "right", "up", "down"];

const capitalise = (word: string) => word[0].toUpperCase() + word.slice(1);

export interface WorkspaceCommandDeps {
  readonly workspace: Workspace;
  readonly focusedPaneId: PaneId;
  readonly dispatch: (action: WorkspaceAction) => void;
  /** Overlay controls the reducer knows nothing about. */
  readonly openWidgetPicker: () => void;
  readonly openWindowSwitcher: () => void;
  readonly openPaneMenu: () => void;
  /** Show the focused window's pane headers for a few seconds. */
  readonly peekPaneHeader: () => void;
  readonly renameActiveWindow: () => void;
  /** True while the pane-target overlay owns the keyboard. */
  readonly targeting: boolean;
  /** True while the staged widget opener is up. */
  readonly openerOpen: boolean;
  readonly resolveTarget: (pane: PaneId) => void;
  /** Send to the pane wearing this number in the overlay. */
  readonly resolveTargetByOrdinal: (ordinal: number) => void;
  readonly resolveTargetSplit: (dir: "row" | "col") => void;
  readonly resolveTargetNewWindow: () => void;
  readonly cancelTargeting: () => void;
}

export function useWorkspaceCommands(deps: WorkspaceCommandDeps): void {
  const { dispatch, focusedPaneId } = deps;

  const handlers = useMemo<Record<string, CommandHandler>>(() => {
    const map: Record<string, CommandHandler> = {
      "workspace.splitRight": () => dispatch({ type: "splitPane", pane: focusedPaneId, dir: "row" }),
      "workspace.splitDown": () => dispatch({ type: "splitPane", pane: focusedPaneId, dir: "col" }),
      "workspace.closePane": () => dispatch({ type: "closePane", pane: focusedPaneId }),
      "workspace.closeOtherPanes": () => dispatch({ type: "closeOthers", pane: focusedPaneId }),
      "workspace.zoomPane": () => dispatch({ type: "toggleZoom" }),
      "workspace.equalize": () => dispatch({ type: "equalize" }),
      "workspace.cyclePane": () => dispatch({ type: "cycleFocus", step: 1 }),

      "workspace.newWindow": () => dispatch({ type: "newWindow" }),
      "workspace.closeWindow": () =>
        dispatch({ type: "closeWindow", window: deps.workspace.activeWindowId }),
      "workspace.nextWindow": () => dispatch({ type: "stepWindow", step: 1 }),
      "workspace.previousWindow": () => dispatch({ type: "stepWindow", step: -1 }),
      "workspace.movePaneToNewWindow": () =>
        dispatch({ type: "movePaneToWindow", pane: focusedPaneId, window: "new" }),

      "workspace.toggleRail": () => dispatch({ type: "toggleChrome", level: "rail" }),
      // The header is peeked, not toggled: it shows itself for a few seconds
      // and withdraws, so there is nothing to switch back off. Zen means what
      // the name says — one pane, the whole shell.
      "workspace.revealPaneHeader": deps.peekPaneHeader,
      "workspace.toggleZen": () => withViewTransition(() => dispatch({ type: "toggleZen" })),

      "workspace.openWidget": deps.openWidgetPicker,
      "workspace.windowSwitcher": deps.openWindowSwitcher,
      "workspace.paneMenu": deps.openPaneMenu,
      "workspace.renameWindow": deps.renameActiveWindow,

      "workspace.targetAccept": () => deps.resolveTarget(focusedPaneId),
      "workspace.targetCancel": deps.cancelTargeting,
      "workspace.targetSplitDown": () => deps.resolveTargetSplit("col"),
      "workspace.targetSplitRight": () => deps.resolveTargetSplit("row"),
      "workspace.targetNewWindow": deps.resolveTargetNewWindow,
    };

    for (const direction of DIRECTIONS) {
      map[`workspace.focus${capitalise(direction)}`] = () =>
        dispatch({ type: "focusDirection", dir: direction });
      map[`workspace.movePane${capitalise(direction)}`] = () =>
        dispatch({ type: "movePane", dir: direction });
    }

    for (const ordinal of ORDINALS) {
      map[`workspace.focusPane${ordinal}`] = () => dispatch({ type: "focusOrdinal", ordinal });
      // While targeting, the same digit sends rather than focuses — the
      // overlay is showing these very numbers, so they must mean what it says.
      map[`workspace.targetPane${ordinal}`] = () => deps.resolveTargetByOrdinal(ordinal);
    }

    return map;
  }, [deps, dispatch, focusedPaneId]);

  useCommands(handlers);

  /**
   * Publish the tree to the shell's directional navigation.
   *
   * `focusPane` moves DOM focus itself rather than trusting the reducer to do
   * it: coming back from the sidebar usually re-focuses the pane that is
   * *already* the focused one, so the dispatch is a no-op, no render happens,
   * and the effect inside `Pane` that adopts focus never runs.
   */
  useNavTree({
    snapshot: () => {
      const active =
        deps.workspace.windows.find((w) => w.id === deps.workspace.activeWindowId) ??
        deps.workspace.windows[0];
      return active ? { root: active.root, focused: active.focusedPaneId } : null;
    },
    focusPane: (pane) => {
      dispatch({ type: "focusPane", pane });
      document.querySelector<HTMLElement>(`[data-pane-id="${pane}"]`)?.focus({ preventScroll: true });
    },
  });

  // `workspaceTargeting` is what scopes the overlay's bare digits and letters,
  // exactly as `focus==git` scopes the git panel's.
  useWhenContext({
    workspaceTargeting: deps.targeting,
    workspaceOpener: deps.openerOpen,
    // Escape leaves Zen. Scoped, because Escape means something else
    // everywhere else.
    workspaceZen: deps.workspace.chrome.zen,
  });
}
