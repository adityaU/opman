import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WindowLayer } from "./WindowLayer";
import { WindowRail } from "./WindowRail";
import { WindowView } from "./WindowView";
import { withViewTransition } from "./viewTransition";
import type { WorkspaceAction } from "./reducer";
import type {
  PaneId,
  PaneNode,
  SplitId,
  WidgetKind,
  WidgetState,
  WindowId,
  Workspace,
} from "./types";

/**
 * The desktop workspace: the window rail, and the pane trees beside it.
 *
 * Every window the user has opened this session keeps a mounted tree; only the
 * active one is visible (see WindowLayer). That is what makes switching
 * windows non-destructive — the widgets never unmount, so nothing they hold
 * outside the reducer is lost.
 *
 * Widgets are injected rather than imported. `renderWidget` is supplied by the
 * shell, which is what keeps this directory free of the app's SSE, session and
 * project plumbing — and what lets a pane hold anything the shell can render
 * without this file learning about it.
 *
 * Every callback handed to `WindowView` has to keep one identity for the life
 * of the workspace, and `renderWidget` and `describePane` with it: a switch
 * changes `activeWindowId` and nothing else, so a memoised window can only bail
 * out of re-rendering if none of its props moved. That is the entire cost model
 * of a window switch — two class names and a focus call, or three transcripts
 * re-parsed through react-markdown.
 */

/** Matches the `.wsp-window` opacity transition in workspace-motion.css. */
const SWITCH_MS = 160;

export interface PaneContext {
  readonly projectName: string;
  readonly subtitle: string | null;
  readonly busy: boolean;
}

export interface WorkspaceRootProps {
  readonly workspace: Workspace;
  readonly dispatch: (action: WorkspaceAction) => void;
  readonly renderWidget: (widget: WidgetState, pane: PaneNode, focused: boolean) => React.ReactNode;
  /** Project name, live subtitle and busy state for a pane's chrome. */
  readonly describePane: (widget: WidgetState | null) => PaneContext;
  /** Windows with a busy agent inside — drives the rail and spine pulse. */
  readonly busyWindows: ReadonlySet<WindowId>;
  /** Panes busy for a reason the widget itself reports, e.g. a running command. */
  readonly busyPanes: ReadonlySet<PaneId>;
  readonly onOpenWidget: (pane: PaneId, kind: WidgetKind) => void;
  readonly onPaneMenu: (pane: PaneId, anchor: HTMLElement) => void;
  readonly onRenameWindow: (id: WindowId) => void;
  /** Widget drag: the pane a drag started from, and the two ends of one. */
  readonly dragSourcePane: PaneId | null;
  readonly onDragWidget: (pane: PaneId) => void;
  readonly onDragWidgetEnd: () => void;
}

export const WorkspaceRoot: React.FC<WorkspaceRootProps> = function WorkspaceRoot({
  workspace,
  dispatch,
  renderWidget,
  describePane,
  busyWindows,
  busyPanes,
  onOpenWidget,
  onPaneMenu,
  onRenameWindow,
  dragSourcePane,
  onDragWidget,
  onDragWidgetEnd,
}) {
  /**
   * Which windows have a mounted tree.
   *
   * Every window the user has visited stays mounted so its widgets keep their
   * state (see WindowLayer). Ones they have not opened this session — a
   * six-window layout restored from storage, say — are not mounted at all, so
   * restoring a workspace does not open six windows' worth of SSE streams and
   * terminals to show one.
   */
  const [visited, setVisited] = useState<ReadonlySet<WindowId>>(
    () => new Set([workspace.activeWindowId]),
  );

  useEffect(() => {
    setVisited((previous) =>
      previous.has(workspace.activeWindowId)
        ? previous
        : new Set(previous).add(workspace.activeWindowId),
    );
  }, [workspace.activeWindowId]);

  // Closed windows fall out here rather than being pruned from `visited`:
  // a stale id in the set costs nothing and cannot resurrect anything.
  const live = useMemo(
    () => workspace.windows.filter((candidate) => visited.has(candidate.id)),
    [visited, workspace.windows],
  );

  /**
   * Zen is published on <html> rather than plumbed down as a prop.
   *
   * The surfaces it has to silence — the sidebar column, the resize handle,
   * the status bar — are siblings of the workspace, not descendants, and
   * `usePanelState` owns the sidebar's own visibility. An attribute lets the
   * cascade reach all of them without giving the workspace a second opinion
   * about a boolean it does not own.
   */
  useEffect(() => {
    const root = document.documentElement;
    if (workspace.chrome.zen) root.setAttribute("data-wsp-zen", "");
    else root.removeAttribute("data-wsp-zen");
    return () => root.removeAttribute("data-wsp-zen");
  }, [workspace.chrome.zen]);

  /**
   * Promote the window layers to their own compositing layers, but only while a
   * switch is actually in flight.
   *
   * The switch cross-fades two full-screen trees. Without a promotion the
   * browser repaints both on every frame of it, which was the most expensive
   * thing about changing windows; with one, the fade is a compositor property
   * and costs nothing. `will-change` left on permanently would hold a
   * full-screen layer per mounted window forever, which trades the frame cost
   * for GPU memory that grows with the desk. An attribute for the duration of
   * the transition buys the frames and gives the memory back.
   *
   * Written to the DOM rather than held in state: it is presentation with a
   * timer on it, and two extra renders of the whole workspace to say "still
   * fading" would be the very work this exists to avoid.
   */
  const bodyRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    body.dataset.wspSwitching = "";
    const timer = setTimeout(() => delete body.dataset.wspSwitching, SWITCH_MS);
    return () => clearTimeout(timer);
  }, [workspace.activeWindowId]);

  const onFocus = useCallback((pane: PaneId) => dispatch({ type: "focusPane", pane }), [dispatch]);

  // Structural changes go through a view transition: closing a pane has no
  // element left to animate once React unmounts it, and what wants animating
  // is the siblings growing into the space anyway. Focus and resize are left
  // alone — one is not a structural change and the other is a live drag.
  const onClose = useCallback(
    (pane: PaneId) => withViewTransition(() => dispatch({ type: "closePane", pane })),
    [dispatch],
  );
  const onSplit = useCallback(
    (pane: PaneId, dir: "row" | "col") =>
      withViewTransition(() => dispatch({ type: "splitPane", pane, dir })),
    [dispatch],
  );
  const onResize = useCallback(
    (split: SplitId, index: number, delta: number) =>
      dispatch({ type: "resize", split, index, delta }),
    [dispatch],
  );
  const onEqualize = useCallback(() => dispatch({ type: "equalize" }), [dispatch]);
  const onActivate = useCallback(
    (id: WindowId) => dispatch({ type: "activateWindow", window: id }),
    [dispatch],
  );
  const onNewWindow = useCallback(() => dispatch({ type: "newWindow" }), [dispatch]);
  const onToggleRail = useCallback(
    () => dispatch({ type: "toggleChrome", level: "rail" }),
    [dispatch],
  );
  const onToggleZen = useCallback(
    () => withViewTransition(() => dispatch({ type: "toggleZen" })),
    [dispatch],
  );

  return (
    <div className="wsp-root" data-workspace="desktop">
      <div className="wsp-body" ref={bodyRef}>
        {live.map((win) => (
          <WindowLayer
            key={win.id}
            active={win.id === workspace.activeWindowId}
            focusedPaneId={win.focusedPaneId}
          >
            <WindowView
              win={win}
              describePane={describePane}
              renderWidget={renderWidget}
              busyPanes={busyPanes}
              showHeaders={workspace.chrome.paneHeaders}
              zen={workspace.chrome.zen}
              dragSourcePane={dragSourcePane}
              onFocus={onFocus}
              onSplit={onSplit}
              onClose={onClose}
              onMenu={onPaneMenu}
              onToggleZen={onToggleZen}
              onOpenWidget={onOpenWidget}
              onDragWidget={onDragWidget}
              onDragWidgetEnd={onDragWidgetEnd}
              onResize={onResize}
              onEqualize={onEqualize}
            />
          </WindowLayer>
        ))}
      </div>
      {/* Last, so the rail sits on the right: the panes are what the user is
          working in, and the window list belongs on the outside edge with the
          other navigation rather than between the sidebar and the work. */}
      <WindowRail
        windows={workspace.windows}
        activeWindowId={workspace.activeWindowId}
        expanded={workspace.chrome.rail}
        busyWindows={busyWindows}
        onActivate={onActivate}
        onNewWindow={onNewWindow}
        onRename={onRenameWindow}
        onToggle={onToggleRail}
      />
    </div>
  );
};
