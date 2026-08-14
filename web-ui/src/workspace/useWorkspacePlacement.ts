import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { formatTime } from "../sidebar/formatTime";
import { createShell } from "../terminal-panel/useShells";
import type { PtySession } from "../api";
import { EMPTY_DRAFT, toWidget, type OpenerDraft, type StepId } from "./opener/steps";
import { browserIdForProject } from "../api/browser";
import { planBrowserOpen } from "./browserOpen";
import { nextRevealSeq, planFileOpen, projectForFile } from "./fileOpen";
import type { DropEdge } from "./move";
import { findPane } from "./tree";
import { withViewTransition } from "./viewTransition";
import { paneByOrdinal } from "./nav";
import {
  WIDGET_KINDS,
  type FileOpenRequest,
  type Node,
  type PaneId,
  type PaneNode,
  type WidgetKind,
  type WidgetForPane,
  type WidgetState,
  type WindowId,
} from "./types";
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
  // Kept apart from `targeting`: the targeting commands (1-9, s/v, n) are gated
  // on it, and a pointer drag must not arm a keymap the user cannot see.
  const [dragSource, setDragSource] = useState<PaneId | null>(null);

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
        const projectPath = projects.length === 1 ? projects[0].path : null;
        setOpener({ draft: { ...EMPTY_DRAFT, kind, projectPath } });
        return;
      }
      if (projects.length === 1) {
        place(widgetFor(kind, projects[0].path, pane), pane);
        return;
      }
      dispatch({ type: "focusPane", pane });
      // Carry the kind through: the four buttons *are* step one.
      setOpener({ draft: { ...EMPTY_DRAFT, kind } });
    },
    [dispatch, place, projects],
  );

  // The shell's "open a files/terminal/git pane here" commands land in the
  // focused pane, which the caller does not know. A ref keeps the published
  // bridge callbacks stable while the focused pane moves under them.
  const openHere = useRef({ onOpenWidgetKind, focusedPaneId, panes, projects });
  openHere.current = { onOpenWidgetKind, focusedPaneId, panes, projects };
  const openKindHere = useCallback((kind: WidgetKind) => {
    openHere.current.onOpenWidgetKind(openHere.current.focusedPaneId, kind);
  }, []);

  /**
   * Reveal a file, asked for from outside the workspace — a path clicked in a
   * tool card, or an MCP editor-open event. `planFileOpen` decides where it
   * goes; this only carries the answer to the reducer.
   */
  const openFileHere = useCallback(
    (path: string, line: number | null) => {
      const { focusedPaneId: focused, panes: paneList, projects: projectList } = openHere.current;
      const open: FileOpenRequest = { path, line, seq: nextRevealSeq() };
      const plan = planFileOpen(open, paneList, focused, projectList);
      if (plan.action === "place") {
        place(plan.widget, plan.pane);
        return;
      }
      dispatch({ type: "splitPane", pane: plan.pane, dir: "row", widget: plan.widget });
    },
    [dispatch, place],
  );

  /**
   * Reveal a file, but let the user say where it lands.
   *
   * The sibling of `openFileHere`, for the case where the caller is *in* the
   * editor: jumping to a definition is as often "show me this beside what I am
   * reading" as "replace what I am reading", and only the reader knows which.
   * The pane is answered by the same overlay a session click uses, so the
   * vocabulary — a number, a split, a new window — is one the user already has.
   */
  const openFileWhere = useCallback(
    (path: string, line: number | null, label: string) => {
      const { focusedPaneId: focused, panes: paneList, projects: projectList } = openHere.current;
      const open: FileOpenRequest = { path, line, seq: nextRevealSeq() };
      const projectPath = projectForFile(path, projectList, undefined)
        || paneList.find((pane) => pane.id === focused)?.widget?.projectPath
        || "";
      const widgetForPane: WidgetForPane = (pane) => ({
        kind: "files", projectPath, sessionId: pane, open,
      });
      armOrPlace({ widget: widgetForPane(focused), widgetForPane, label });
    },
    [armOrPlace],
  );

  /**
   * Reveal the browser an agent just drove somewhere. Same contract as
   * `openFileHere`: the caller knows what, the workspace knows where.
   */
  const openBrowserHere = useCallback(
    (projectPath: string, url: string) => {
      const { focusedPaneId: focused, panes: paneList } = openHere.current;
      const plan = planBrowserOpen(projectPath, url, paneList, focused);
      if (plan.action === "place") {
        place(plan.widget, plan.pane);
        return;
      }
      dispatch({ type: "splitPane", pane: plan.pane, dir: "row", widget: plan.widget });
    },
    [dispatch, place],
  );

  // Re-read the shells each time the opener opens, so the list it offers is
  // what is running now rather than what was running last time.
  const refreshShells = shells.refresh;
  useEffect(() => {
    if (opener) refreshShells();
  }, [opener, refreshShells]);

  const choicesFor = useCallback(
    (step: StepId, draft: OpenerDraft): readonly OpenerChoice[] => {
      if (step === "kind") {
        return WIDGET_KINDS.map((kind) => ({ value: kind, label: KIND_LABEL[kind] }));
      }
      if (step === "project") {
        return projects.map((p) => ({ value: p.path, label: p.name, hint: p.path }));
      }
      if (step === "shell") {
        return shellChoices(shells.shells, draft.projectPath);
      }
      // Recency-sorted by `sessionsFor`; "New session" is pinned above it
      // because it is the answer to a different question than "which one".
      const sessions = draft.projectPath ? sessionsFor(draft.projectPath) : [];
      return [
        { value: null, label: "New session", hint: "created on first send" },
        ...sessions.map((s) => ({ value: s.id, label: s.title, hint: formatTime(s.updated) })),
      ];
    },
    [projects, sessionsFor, shells],
  );

  const onOpenerDone = useCallback(
    async (draft: OpenerDraft) => {
      setOpener(null);
      if (!draft.kind || !draft.projectPath) return;

      // "New shell" from the shell step is answered by actually starting one,
      // so the pane opens straight onto a prompt rather than onto a second
      // picker asking the question the modal just asked.
      let resolved = draft;
      if (draft.kind === "terminal" && draft.ptyId === null) {
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
      else armOrPlace({ widget, widgetForPane: createWidget, label: describeWidget(widget, projects) });
    },
    [armOrPlace, focusedPaneId, panes, place, projects],
  );

  /**
   * A pane dropped on another pane. The edge decides which drop it is; the
   * reducer owns both, so this only has to end the gesture.
   *
   * Through a view transition, for the reason `WorkspaceRoot` gives for closing
   * a pane: an edge drop re-seats one pane and leaves its old siblings growing
   * into the space, and there is no element left to animate once React has
   * moved it.
   */
  const dropWidget = useCallback(
    (pane: PaneId, edge: DropEdge) => {
      if (dragSource) {
        withViewTransition(() =>
          dispatch({ type: "dropPane", pane: dragSource, target: pane, edge }),
        );
      }
      setDragSource(null);
    },
    [dispatch, dragSource],
  );

  /** The same drag, let go over another window — or over "new window". */
  const dropOnWindow = useCallback(
    (window: WindowId | "new") => {
      if (dragSource) dispatch({ type: "movePaneToWindow", pane: dragSource, window });
      setDragSource(null);
    },
    [dispatch, dragSource],
  );
  const endDrag = useCallback(() => setDragSource(null), []);

  const draggedLabel = useMemo(() => {
    if (!dragSource) return "";
    const pane = findPane(root, dragSource);
    return pane?.widget ? describeWidget(pane.widget, projects) : "";
  }, [dragSource, projects, root]);

  const openWidgetPicker = useCallback(() => setOpener({ draft: EMPTY_DRAFT }), []);
  const closeOpener = useCallback(() => setOpener(null), []);

  return {
    opener,
    openWidgetPicker,
    closeOpener,
    choicesFor,
    onOpenerDone,
    onOpenWidgetKind,
    openKindHere,
    openFileHere,
    openFileWhere,
    openBrowserHere,
    armOrPlace,
    resolveTarget,
    resolveTargetByOrdinal,
    resolveTargetSplit,
    resolveTargetNewWindow,
    dragSource,
    setDragSource,
    endDrag,
    dropWidget,
    dropOnWindow,
    draggedLabel,
  };
}

// ── Helpers ─────────────────────────────────────────────

const KIND_LABEL: Readonly<Record<WidgetKind, string>> = {
  chat: "Chat",
  files: "Files",
  terminal: "Terminal",
  git: "Git",
  browser: "Browser",
};

function widgetFor(kind: WidgetKind, projectPath: string, paneId: PaneId): WidgetState {
  switch (kind) {
    case "chat":
      return { kind: "chat", projectPath, sessionId: null, engine: null };
    case "files":
      return { kind: "files", projectPath, sessionId: paneId, open: null };
    case "terminal":
      // No shell chosen: the pane shows the picker, listing what is already
      // running here alongside "new shell".
      return { kind: "terminal", projectPath, ptyId: null };
    case "git":
      return { kind: "git", projectPath };
    case "browser":
      return {
        kind: "browser",
        projectPath,
        browserId: browserIdForProject(projectPath),
        url: null,
        reveal: 0,
      };
  }
}

/**
 * The running shells in one project, busiest first, under "New shell".
 *
 * Busy first because the shell someone is looking for is usually the one with
 * work in it; a numbered label alone gives no reason to prefer any of them.
 */
function shellChoices(
  shells: readonly PtySession[],
  projectPath: string | null,
): readonly OpenerChoice[] {
  const mine = projectPath ? shells.filter((shell) => shell.project === projectPath) : [];
  const ordered = [...mine].sort((a, b) => {
    if (a.activity !== b.activity) return a.activity === "running" ? -1 : 1;
    return a.label.localeCompare(b.label, undefined, { numeric: true });
  });
  return [
    { value: null, label: "New shell", hint: "started in the project root" },
    ...ordered.map((shell) => ({
      value: shell.id,
      label: shell.label,
      hint: shell.activity === "running" ? "running a command" : "idle",
      busy: shell.activity === "running",
    })),
  ];
}

function describeWidget(widget: WidgetState, projects: readonly WorkspaceProject[]): string {
  const project = projects.find((candidate) => candidate.path === widget.projectPath);
  return `${KIND_LABEL[widget.kind]} · ${project?.name ?? widget.projectPath}`;
}
