import type { PaneMenuItem } from "./PaneMenu";
import type { WorkspaceAction } from "./reducer";
import type { PaneId } from "./types";

/**
 * The pane header's overflow menu.
 *
 * Every entry is a keymap command with a chord already, so the menu is a
 * discoverability surface rather than a second way to do things: the shortcut
 * is shown beside each one precisely so it stops being needed.
 */
export function paneMenuItems(
  pane: PaneId,
  dispatch: (action: WorkspaceAction) => void,
  total: number,
): PaneMenuItem[] {
  const alone = total < 2;
  return [
    { id: "split-right", label: "Split right", shortcut: "⌘\\", run: () => dispatch({ type: "splitPane", pane, dir: "row" }) },
    { id: "split-down", label: "Split down", shortcut: "⌘K ⌘\\", run: () => dispatch({ type: "splitPane", pane, dir: "col" }) },
    { id: "zoom", label: "Zoom", shortcut: "⌘K Z", disabled: alone, run: () => dispatch({ type: "toggleZoom" }) },
    { id: "equalize", label: "Reset sizes", shortcut: "⌘K =", run: () => dispatch({ type: "equalize" }) },
    { id: "to-window", label: "Move to new window", disabled: alone, run: () => dispatch({ type: "movePaneToWindow", pane, window: "new" }) },
    { id: "only", label: "Close other panes", shortcut: "⌘K U", disabled: alone, run: () => dispatch({ type: "closeOthers", pane }) },
    { id: "close", label: "Close pane", shortcut: "⌘K Q", danger: true, disabled: alone, run: () => dispatch({ type: "closePane", pane }) },
  ];
}
