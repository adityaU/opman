import { useCallback, useRef } from "react";
import { planBrowserOpen } from "./browserOpen";
import { nextRevealSeq, planFileOpen, projectForFile, type FileOpenPlan } from "./fileOpen";
import { planForeignOpen } from "./foreignOpen";
import type { WorkspaceAction } from "./reducer";
import type { TargetRequest } from "./target/useTargeting";
import type { WorkspaceProject } from "./DesktopWorkspace";
import type { FileOpenRequest, PaneId, PaneNode, WidgetForPane, WidgetState } from "./types";

/**
 * Requests that arrive from outside the workspace already knowing *what* —
 * a path clicked in a tool card, an MCP editor or browser event, a widget an
 * embedded pane forwarded — and need the workspace to answer *where*.
 *
 * Split from `useWorkspacePlacement`, which answers the user's own gestures.
 * The planners decide; this only carries their answer to the reducer. Read
 * through a ref so the callbacks keep one identity while focus moves under
 * them: they are published to the shell in an effect.
 */

export interface ExternalOpenDeps {
  readonly dispatch: (action: WorkspaceAction) => void;
  readonly place: (widget: WidgetState, pane: PaneId) => void;
  readonly armOrPlace: (request: TargetRequest) => void;
  readonly focusedPaneId: PaneId;
  readonly panes: readonly PaneNode[];
  readonly projects: readonly WorkspaceProject[];
}

export function useExternalOpens(deps: ExternalOpenDeps) {
  const { armOrPlace, dispatch, place } = deps;
  const latest = useRef(deps);
  latest.current = deps;

  const carryOut = useCallback(
    (plan: FileOpenPlan) => {
      if (plan.action === "place") {
        place(plan.widget, plan.pane);
        return;
      }
      dispatch({ type: "splitPane", pane: plan.pane, dir: "row", widget: plan.widget });
    },
    [dispatch, place],
  );

  /** Reveal a file: in the files pane already open on it, or in a new split. */
  const openFileHere = useCallback(
    (path: string, line: number | null) => {
      const { focusedPaneId, panes, projects } = latest.current;
      const open: FileOpenRequest = { path, line, seq: nextRevealSeq() };
      carryOut(planFileOpen(open, panes, focusedPaneId, projects));
    },
    [carryOut],
  );

  /**
   * Reveal a file, but let the user say where it lands.
   *
   * For the case where the caller is *in* the editor: jumping to a definition
   * is as often "show me this beside what I am reading" as "replace what I am
   * reading", and only the reader knows which. The pane is answered by the same
   * overlay a session click uses.
   */
  const openFileWhere = useCallback(
    (path: string, line: number | null, label: string) => {
      const { focusedPaneId, panes, projects } = latest.current;
      const open: FileOpenRequest = { path, line, seq: nextRevealSeq() };
      const focusedWidget = panes.find((pane) => pane.id === focusedPaneId)?.widget;
      const projectPath = projectForFile(path, projects, undefined)
        || (focusedWidget?.server ? "" : focusedWidget?.projectPath)
        || "";
      const widgetForPane: WidgetForPane = (pane) => ({
        kind: "files", projectPath, sessionId: pane, open,
      });
      armOrPlace({ widget: widgetForPane(focusedPaneId), widgetForPane, label });
    },
    [armOrPlace],
  );

  /** Reveal the browser an agent just drove somewhere. */
  const openBrowserHere = useCallback(
    (projectPath: string, url: string) => {
      const { focusedPaneId, panes } = latest.current;
      carryOut(planBrowserOpen(projectPath, url, panes, focusedPaneId));
    },
    [carryOut],
  );

  /**
   * Place a whole widget an embedded pane forwarded, already tagged with its
   * server. A reused pane is also focused: the embed asked to *show* this, and
   * an already-open pane elsewhere on screen would otherwise look like nothing
   * happened.
   */
  const openForeignWidget = useCallback(
    (widget: WidgetState) => {
      const { focusedPaneId, panes } = latest.current;
      const plan = planForeignOpen(widget, panes, focusedPaneId, nextRevealSeq());
      carryOut(plan);
      if (plan.action === "place") dispatch({ type: "focusPane", pane: plan.pane });
    },
    [carryOut, dispatch],
  );

  return { openFileHere, openFileWhere, openBrowserHere, openForeignWidget };
}
