import { useCallback, useMemo, useRef, useState } from "react";
import { formatTime } from "../sidebar/formatTime";
import { EMPTY_DRAFT, toWidget, type OpenerDraft, type StepId } from "./opener/steps";
import { nextFileOpenSeq, planFileOpen } from "./fileOpen";
import { findPane } from "./tree";
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
} from "./types";
import type { OpenerChoice } from "./opener/WidgetOpener";
import type { WorkspaceAction } from "./reducer";
import type { TargetRequest } from "./target/useTargeting";
import type { WorkspaceProject } from "./DesktopWorkspace";

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
}

export function useWorkspacePlacement(deps: PlacementDeps) {
  const { dispatch, focusedPaneId, panes, projects, root, sessionsFor, targeting } = deps;

  const [opener, setOpener] = useState<{ draft: OpenerDraft } | null>(null);
  // Kept apart from `targeting`: the targeting commands (1-9, s/v, n) are gated
  // on it, and a pointer drag must not arm a keymap the user cannot see.
  const [dragSource, setDragSource] = useState<PaneId | null>(null);

  const place = useCallback(
    (widget: WidgetState, pane: PaneId) => dispatch({ type: "setWidget", pane, widget }),
    [dispatch],
  );

  const widgetForRequest = useCallback(
    (request: TargetRequest, pane: PaneId) => request.widgetForPane?.(pane) ?? request.widget,
    [],
  );

  /**
   * Ask which pane — unless the question has only one answer.
   *
   * With a single pane the overlay was a whole extra gesture between the user
   * and the thing they clicked: it dimmed the screen to offer one slot, marked
   * "1", which they then had to hit. Asking before replacing is worth a click
   * when there is somewhere else the widget could have gone; when there is
   * not, it is just a click.
   */
  const armOrPlace = useCallback(
    (request: TargetRequest) => {
      if (panes.length === 1) place(widgetForRequest(request, panes[0].id), panes[0].id);
      else targeting.arm(request);
    },
    [panes, place, targeting, widgetForRequest],
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
      const open: FileOpenRequest = { path, line, seq: nextFileOpenSeq() };
      const plan = planFileOpen(open, paneList, focused, projectList);
      if (plan.action === "place") {
        place(plan.widget, plan.pane);
        return;
      }
      dispatch({ type: "splitPane", pane: plan.pane, dir: "row", widget: plan.widget });
    },
    [dispatch, place],
  );

  const choicesFor = useCallback(
    (step: StepId, draft: OpenerDraft): readonly OpenerChoice[] => {
      if (step === "kind") {
        return WIDGET_KINDS.map((kind) => ({ value: kind, label: KIND_LABEL[kind] }));
      }
      if (step === "project") {
        return projects.map((p) => ({ value: p.path, label: p.name, hint: p.path }));
      }
      // Recency-sorted by `sessionsFor`; "New session" is pinned above it
      // because it is the answer to a different question than "which one".
      const sessions = draft.projectPath ? sessionsFor(draft.projectPath) : [];
      return [
        { value: null, label: "New session", hint: "created on first send" },
        ...sessions.map((s) => ({ value: s.id, label: s.title, hint: formatTime(s.updated) })),
      ];
    },
    [projects, sessionsFor],
  );

  const onOpenerDone = useCallback(
    (draft: OpenerDraft) => {
      setOpener(null);
      if (!draft.kind || !draft.projectPath) return;
      const createWidget: WidgetForPane = (pane) => {
        const widget = toWidget(draft, pane);
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

  const dropWidget = useCallback(
    (pane: PaneId) => {
      if (dragSource) dispatch({ type: "swapWidgets", from: dragSource, to: pane });
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
    armOrPlace,
    resolveTarget,
    resolveTargetByOrdinal,
    resolveTargetSplit,
    resolveTargetNewWindow,
    dragSource,
    setDragSource,
    endDrag,
    dropWidget,
    draggedLabel,
  };
}

// ── Helpers ─────────────────────────────────────────────

const KIND_LABEL: Readonly<Record<WidgetKind, string>> = {
  chat: "Chat",
  files: "Files",
  terminal: "Terminal",
  git: "Git",
};

function widgetFor(kind: WidgetKind, projectPath: string, paneId: PaneId): WidgetState {
  switch (kind) {
    case "chat":
      return { kind: "chat", projectPath, sessionId: null, engine: null };
    case "files":
      return { kind: "files", projectPath, sessionId: paneId, open: null };
    case "terminal":
      return { kind: "terminal", projectPath, ptyIds: [] };
    case "git":
      return { kind: "git", projectPath };
  }
}

function describeWidget(widget: WidgetState, projects: readonly WorkspaceProject[]): string {
  const project = projects.find((candidate) => candidate.path === widget.projectPath);
  return `${KIND_LABEL[widget.kind]} · ${project?.name ?? widget.projectPath}`;
}
