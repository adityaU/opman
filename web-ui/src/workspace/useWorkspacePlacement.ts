import { useCallback, useEffect, useRef, useState } from "react";
import { createShell } from "../terminal-panel/useShells";
import { EMPTY_DRAFT, toWidget, type OpenerDraft, type StepId } from "./opener/steps";
import { choicesForStep, describeWidget, widgetFor } from "./opener/choices";
import { useServerProjects } from "./servers/useServerProjects";
import { useExternalOpens } from "./useExternalOpens";
import { usePaneDrag } from "./usePaneDrag";
import { paneByOrdinal } from "./nav";
import type { Node, PaneId, PaneNode, WidgetKind, WidgetForPane, WidgetState } from "./types";
import type { OpenerChoice } from "./opener/WidgetOpener";
import type { WorkspaceAction } from "./reducer";
import type { TargetRequest } from "./target/useTargeting";
import type { WorkspaceProject } from "./DesktopWorkspace";
import type { ShellLabels } from "./useShellLabels";

/**
 * Choosing what to open, and choosing where it goes.
 *
 * One hook rather than two because the two questions are the same gesture: the
 * staged opener answers "what", the target overlay answers "where", and both
 * are skipped when they have only one possible answer. Splitting them would
 * mean threading `place` and the draft between two hooks that always run
 * together.
 */

interface Targeting {
  readonly arm: (request: TargetRequest) => void;
  readonly take: () => TargetRequest | null;
}

export interface PlacementDeps {
  readonly projects: readonly WorkspaceProject[];
  readonly sessionsFor: (path: string) => readonly { id: string; title: string; updated: number }[];
  readonly dispatch: (action: WorkspaceAction) => void;
  readonly targeting: Targeting;
  readonly root: Node;
  readonly focusedPaneId: PaneId;
  readonly panes: readonly PaneNode[];
  /**
   * The running shells, from the cache the pane menu also reads. Passed in
   * rather than fetched here: both surfaces name the same shells, and two
   * fetches could name them differently.
   */
  readonly shells: ShellLabels;
}

export function useWorkspacePlacement(deps: PlacementDeps) {
  const { dispatch, focusedPaneId, panes, projects, root, sessionsFor, shells, targeting } = deps;

  const [opener, setOpener] = useState<{ draft: OpenerDraft } | null>(null);

  // Every server's projects once there is more than one; with one, the plain
  // list and no fetching at all.
  const servers = useServerProjects(projects, opener !== null);
  const { multi, serverName } = servers;
  const describe = useCallback(
    (widget: WidgetState) => describeWidget(widget, projects, serverName),
    [projects, serverName],
  );

  // Every route through this hook is the user pointing a pane at something, so
  // all of it records: the opener, a tool-card file link, a dropped target, a
  // session picked in the sidebar.
  const place = useCallback(
    (widget: WidgetState, pane: PaneId) => dispatch({ type: "openWidget", pane, widget }),
    [dispatch],
  );

  const widgetForRequest = useCallback(
    (request: TargetRequest, pane: PaneId) => request.widgetForPane?.(pane) ?? request.widget,
    [],
  );

  /**
   * Always ask which pane.
   *
   * This used to place straight into the only pane when there was one, on the
   * grounds that a one-slot overlay is ceremony. It is not: with a single pane
   * the answer the user most often wants is a *split* or a new window, and
   * skipping the question took those away at exactly the moment they mattered
   * most. One gesture that always means the same thing beats one that
   * sometimes answers itself.
   */
  const armOrPlace = useCallback(
    (request: TargetRequest) => targeting.arm(request),
    [targeting],
  );

  const resolveTarget = useCallback(
    (pane: PaneId) => {
      const request = targeting.take();
      if (request) place(widgetForRequest(request, pane), pane);
    },
    [place, targeting, widgetForRequest],
  );

  const resolveTargetByOrdinal = useCallback(
    (ordinal: number) => {
      const pane = paneByOrdinal(root, ordinal);
      if (pane) resolveTarget(pane);
    },
    [resolveTarget, root],
  );

  const resolveTargetSplit = useCallback(
    (dir: "row" | "col") => {
      const request = targeting.take();
      if (request) dispatch({
        type: "splitPane",
        pane: focusedPaneId,
        dir,
        widget: request.widget,
        widgetForPane: request.widgetForPane,
      });
    },
    [dispatch, focusedPaneId, targeting],
  );

  const resolveTargetNewWindow = useCallback(() => {
    const request = targeting.take();
    if (request) dispatch({
      type: "newWindow",
      widget: request.widget,
      widgetForPane: request.widgetForPane,
    });
  }, [dispatch, targeting]);

  /**
   * The inline four-button opener on an empty pane. It answers step one, so
   * the project still has to be chosen — unless there is only one, in which
   * case asking would be ceremony.
   */
  const onOpenWidgetKind = useCallback(
    (pane: PaneId, kind: WidgetKind) => {
      // A terminal always has a third question — which shell — so it stays in
      // the staged opener even with one project; the project step is simply
      // pre-answered. Same flow as chat choosing a session.
      if (kind === "terminal") {
        dispatch({ type: "focusPane", pane });
        const projectPath = projects.length === 1 && !multi ? projects[0].path : null;
        setOpener({ draft: { ...EMPTY_DRAFT, kind, projectPath } });
        return;
      }
      // One project is only "no question" while there is one server to have it on.
      if (projects.length === 1 && !multi) {
        place(widgetFor(kind, projects[0].path, pane), pane);
        return;
      }
      dispatch({ type: "focusPane", pane });
      // Carry the kind through: the four buttons *are* step one.
      setOpener({ draft: { ...EMPTY_DRAFT, kind } });
    },
    [dispatch, multi, place, projects],
  );

  // The shell's "open a files/terminal/git pane here" commands land in the
  // focused pane, which the caller does not know. A ref keeps the published
  // bridge callbacks stable while the focused pane moves under them.
  const openHere = useRef({ onOpenWidgetKind, focusedPaneId });
  openHere.current = { onOpenWidgetKind, focusedPaneId };
  const openKindHere = useCallback((kind: WidgetKind) => {
    openHere.current.onOpenWidgetKind(openHere.current.focusedPaneId, kind);
  }, []);

  const external = useExternalOpens({ dispatch, place, armOrPlace, focusedPaneId, panes, projects });

  // Re-read the shells each time the opener opens, so the list it offers is
  // what is running now rather than what was running last time.
  const refreshShells = shells.refresh;
  useEffect(() => {
    if (opener) refreshShells();
  }, [opener, refreshShells]);

  const { projectChoices, sessionChoices, prefetchSessions } = servers;
  const choicesFor = useCallback(
    (step: StepId, draft: OpenerDraft): readonly OpenerChoice[] =>
      choicesForStep(step, draft, {
        projectChoices,
        sessionsFor,
        remoteSessions: sessionChoices,
        shells: shells.shells,
      }),
    [projectChoices, sessionChoices, sessionsFor, shells],
  );

  /** Another server's sessions are fetched the moment its project is chosen. */
  const onOpenerStep = useCallback(
    (step: StepId, draft: OpenerDraft) => {
      if (step === "session" && draft.server && draft.projectPath) {
        prefetchSessions(draft.server, draft.projectPath);
      }
    },
    [prefetchSessions],
  );

  const onOpenerDone = useCallback(
    async (draft: OpenerDraft) => {
      setOpener(null);
      if (!draft.kind || !draft.projectPath) return;

      // "New shell" from the shell step is answered by actually starting one,
      // so the pane opens straight onto a prompt rather than onto a second
      // picker asking the question the modal just asked.
      let resolved = draft;
      // Another server's pane asks for itself, in that server's own picker.
      if (draft.kind === "terminal" && draft.ptyId === null && !draft.server) {
        const ptyId = await createShell("shell", draft.projectPath).catch(() => null);
        if (!ptyId) return;
        resolved = { ...draft, ptyId };
      }

      const createWidget: WidgetForPane = (pane) => {
        const widget = toWidget(resolved, pane);
        if (!widget) throw new Error("Incomplete widget opener draft");
        return widget;
      };
      const widget = createWidget(focusedPaneId);
      // Straight into the focused pane when it is empty; otherwise ask where,
      // rather than silently replacing something the user is looking at.
      const focused = panes.find((pane) => pane.id === focusedPaneId);
      if (focused && !focused.widget) place(widget, focused.id);
      else armOrPlace({
        widget,
        widgetForPane: createWidget,
        label: describe(widget),
      });
    },
    [armOrPlace, describe, focusedPaneId, panes, place],
  );

  const drag = usePaneDrag(dispatch, root, describe);

  const openWidgetPicker = useCallback(() => setOpener({ draft: EMPTY_DRAFT }), []);
  const closeOpener = useCallback(() => setOpener(null), []);

  return {
    opener,
    openWidgetPicker,
    closeOpener,
    choicesFor,
    onOpenerStep,
    onOpenerDone,
    serverName,
    onOpenWidgetKind,
    openKindHere,
    ...external,
    armOrPlace,
    resolveTarget,
    resolveTargetByOrdinal,
    resolveTargetSplit,
    resolveTargetNewWindow,
    ...drag,
  };
}
