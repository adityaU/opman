import type { PaneMenuItem } from "./PaneMenu";
import type { WorkspaceAction } from "./reducer";
import type { PaneId } from "./types";

/**
 * The pane header's overflow menu.
 *
 * Every entry is a keymap command with a chord already, so the menu is a
 * discoverability surface rather than a second way to do things: the shortcut
 * is shown beside each one precisely so it stops being needed. The row names
 * the command and the menu resolves it — the chord shown is therefore the one
 * that is bound right now, on this platform, in the mode the user chose.
 */
export function paneMenuItems(
  pane: PaneId,
  dispatch: (action: WorkspaceAction) => void,
  total: number,
): PaneMenuItem[] {
  const alone = total < 2;
  return [
    { id: "split-right", label: "Split right", command: "workspace.splitRight", run: () => dispatch({ type: "splitPane", pane, dir: "row" }) },
    { id: "split-down", label: "Split down", command: "workspace.splitDown", run: () => dispatch({ type: "splitPane", pane, dir: "col" }) },
    { id: "zoom", label: "Zoom", command: "workspace.zoomPane", disabled: alone, run: () => dispatch({ type: "toggleZoom" }) },
    { id: "equalize", label: "Reset sizes", command: "workspace.equalize", run: () => dispatch({ type: "equalize" }) },
    { id: "to-window", label: "Move to new window", command: "workspace.movePaneToNewWindow", disabled: alone, run: () => dispatch({ type: "movePaneToWindow", pane, window: "new" }) },
    { id: "only", label: "Close other panes", command: "workspace.closeOtherPanes", disabled: alone, run: () => dispatch({ type: "closeOthers", pane }) },
    { id: "close", label: "Close pane", command: "workspace.closePane", danger: true, disabled: alone, run: () => dispatch({ type: "closePane", pane }) },
  ];
}
