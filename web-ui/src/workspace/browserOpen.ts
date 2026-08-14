/**
 * Where a browser an agent opened should appear.
 *
 * The same shape as `planFileOpen`, and for the same reason: something outside
 * the workspace has decided *what* to show, and only the workspace can decide
 * *where*. Reusing the pane that already holds this project's browser is the
 * whole point — a browser is per project, so a second pane for one would be two
 * views of one tab, which is worse than none.
 */

import { browserIdForProject } from "../api/browser";
import type { PaneId, PaneNode, WidgetState } from "./types";

export type BrowserOpenPlan =
  | { readonly action: "place"; readonly pane: PaneId; readonly widget: WidgetState }
  | { readonly action: "split"; readonly pane: PaneId; readonly widget: WidgetState };

export function planBrowserOpen(
  projectPath: string,
  url: string,
  panes: readonly PaneNode[],
  focusedPaneId: PaneId,
): BrowserOpenPlan {
  const browserId = browserIdForProject(projectPath);
  // `reveal: 0` — this is a fresh request, not a step back through the pane's
  // history, and the panel navigates on its own for a live open.
  const widget: WidgetState = { kind: "browser", projectPath, browserId, url, reveal: 0 };

  // Already on screen: update the URL it remembers and leave the pane where it
  // is. The panel is connected to the same tab, so it is already showing the
  // page — moving or recreating the pane would only lose the user's place.
  const existing = panes.find(
    (pane) => pane.widget?.kind === "browser" && pane.widget.browserId === browserId,
  );
  if (existing) return { action: "place", pane: existing.id, widget };

  // An empty pane is a slot already waiting; splitting it would strand the
  // empty half.
  const focused = panes.find((pane) => pane.id === focusedPaneId);
  if (focused && !focused.widget) return { action: "place", pane: focused.id, widget };

  // Otherwise a column beside the focused pane — a browser is read alongside
  // the work, not on top of it.
  return { action: "split", pane: focusedPaneId, widget };
}
