import type { WorkspaceProject } from "./DesktopWorkspace";
import type { FileOpenRequest, PaneId, PaneNode, WidgetState } from "./types";

/**
 * Where a file revealed from outside the workspace goes.
 *
 * A pure rule rather than a branch inside the hook, for the same reason the
 * reducer is: "which pane shows this file" is the whole of the behaviour, and
 * it should be answerable — and testable — without rendering a workspace.
 */

export type FileOpenPlan =
  /** Write the widget onto an existing pane: one already showing files, or an empty one. */
  | { readonly action: "place"; readonly pane: PaneId; readonly widget: WidgetState }
  /** Split the pane side by side and put the files widget in the new half. */
  | { readonly action: "split"; readonly pane: PaneId; readonly widget: WidgetState };

/**
 * Monotonic across the tab. Two clicks on the same path are two requests, and
 * only the counter tells the panel that the second one happened — without it,
 * asking again for a file the user has since browsed away from changes nothing.
 *
 * Shared with the browser pane, whose tab has the same property for the same
 * reason: it keeps its own page, so an earlier URL written back to it reads as
 * nothing having happened. One counter rather than one per panel, because it
 * only has to rise and a single source cannot disagree with itself.
 */
let sequence = 0;
export function nextRevealSeq(): number {
  sequence += 1;
  return sequence;
}

/**
 * Which project a file belongs to: the longest project root that contains it.
 *
 * Tool cards report relative paths as often as absolute ones, and a relative
 * path matches no root at all — so the fallback is the project of the pane the
 * request came from, which is what the path was written relative to.
 */
export function projectForFile(
  filePath: string,
  projects: readonly WorkspaceProject[],
  fallback: string | undefined,
): string {
  let best = "";
  for (const project of projects) {
    if (project.path.length <= best.length) continue;
    if (filePath === project.path || filePath.startsWith(`${project.path}/`)) best = project.path;
  }
  return best || fallback || projects[0]?.path || "";
}

/**
 * A files pane already on screen is the answer when there is one: the user is
 * looking at it, and a second file tree for the same project would be two
 * copies of one job. Failing that the file needs a pane of its own, and taking
 * the focused one would swap the conversation out from under the click that
 * asked for it — so it lands beside it, in a fresh split.
 *
 * The match is scoped to one project: a files pane resolves paths against its
 * own root, so handing it a file from elsewhere would read as a missing file.
 */
export function planFileOpen(
  open: FileOpenRequest,
  panes: readonly PaneNode[],
  focusedPaneId: PaneId,
  projects: readonly WorkspaceProject[],
): FileOpenPlan {
  const focused = panes.find((pane) => pane.id === focusedPaneId) ?? null;
  const projectPath = projectForFile(open.path, projects, focused?.widget?.projectPath);

  const existing = panes.find(
    (pane) => pane.widget?.kind === "files" && pane.widget.projectPath === projectPath,
  );
  if (existing?.widget?.kind === "files") {
    return { action: "place", pane: existing.id, widget: { ...existing.widget, open } };
  }

  const widget: WidgetState = { kind: "files", projectPath, sessionId: focusedPaneId, open };
  // An empty pane is a slot already waiting; splitting it would leave the empty
  // half behind.
  if (focused && !focused.widget) return { action: "place", pane: focused.id, widget };
  return { action: "split", pane: focusedPaneId, widget };
}
