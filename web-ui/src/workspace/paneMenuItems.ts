import { canStep, peekStep, recentTargets, targetLabel } from "./history";
import type { PaneMenuItem } from "./PaneMenu";
import type { WorkspaceAction } from "./reducer";
import type { PaneHistory } from "./history";
import type { PaneId, WidgetState } from "./types";

/**
 * The pane header's overflow menu.
 *
 * Every entry that can have a chord has one, so the menu is a discoverability
 * surface rather than a second way to do things: the shortcut is shown beside
 * each one precisely so it stops being needed. The row names the command and the
 * menu resolves it — the chord shown is therefore the one that is bound right
 * now, on this platform, in the mode the user chose.
 *
 * The Recent rows are the exception, and have to be: there is no chord for "the
 * third place back", and a jump straight to one is the whole reason to list them
 * rather than make the user press Back three times.
 */

export interface PaneMenuDeps {
  readonly pane: PaneId;
  readonly history: PaneHistory;
  readonly dispatch: (action: WorkspaceAction) => void;
  /** Panes in the window; below two, the pane cannot be closed or zoomed. */
  readonly total: number;
  /** Fresh reveal token, so a jump re-reveals a target the panel already showed. */
  readonly nextSeq: () => number;
  /**
   * A better name for a target than the widget alone can give — a chat session's
   * title, a shell's name. Falls back to `targetLabel` when it has none.
   */
  readonly labelFor: (widget: WidgetState) => string | null;
}

export function paneMenuItems(deps: PaneMenuDeps): PaneMenuItem[] {
  const { dispatch, history, labelFor, nextSeq, pane, total } = deps;
  const alone = total < 2;
  const name = (widget: WidgetState) => labelFor(widget) ?? targetLabel(widget);

  /** "Back" alone when there is nowhere to go; "Back to reducer.ts" when there is. */
  const stepRow = (id: string, label: string, command: PaneMenuItem["command"], step: 1 | -1) => {
    const destination = peekStep(history, step);
    return {
      id,
      label: destination ? `${label} to ${name(destination)}` : label,
      command,
      disabled: !canStep(history, step),
      run: () => dispatch({ type: "historyStep", pane, step, seq: nextSeq() }),
    };
  };

  return [
    stepRow("back", "Back", "workspace.historyBack", -1),
    stepRow("forward", "Forward", "workspace.historyForward", 1),

    { id: "split-right", label: "Split right", command: "workspace.splitRight", run: () => dispatch({ type: "splitPane", pane, dir: "row" }) },
    { id: "split-down", label: "Split down", command: "workspace.splitDown", run: () => dispatch({ type: "splitPane", pane, dir: "col" }) },
    { id: "zoom", label: "Zoom", command: "workspace.zoomPane", disabled: alone, run: () => dispatch({ type: "toggleZoom" }) },
    { id: "equalize", label: "Reset sizes", command: "workspace.equalize", run: () => dispatch({ type: "equalize" }) },
    { id: "to-window", label: "Move to new window", command: "workspace.movePaneToNewWindow", disabled: alone, run: () => dispatch({ type: "movePaneToWindow", pane, window: "new" }) },
    { id: "only", label: "Close other panes", command: "workspace.closeOtherPanes", disabled: alone, run: () => dispatch({ type: "closeOthers", pane }) },
    { id: "close", label: "Close pane", command: "workspace.closePane", danger: true, disabled: alone, run: () => dispatch({ type: "closePane", pane }) },

    // Last, and newest first. The list is where the pane has been rather than a
    // menu of things to do, so it reads as a footnote to the actions above.
    ...recentTargets(history).map(({ index, widget }) => ({
      id: `recent:${index}`,
      label: name(widget),
      run: () => dispatch({ type: "historyJump", pane, index, seq: nextSeq() }),
    })),
  ];
}
