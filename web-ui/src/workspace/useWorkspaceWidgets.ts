import React, { useCallback, useMemo, useRef } from "react";
import { PaneWidget } from "./widgets/PaneWidget";
import { findPane } from "./tree";
import type { WorkspaceAction } from "./reducer";
import type { WorkspaceChatServices } from "./widgets/WorkspaceChatContext";
import type { PaneEngine, PaneId, PaneNode, WidgetState, Workspace } from "./types";

/**
 * The three writes a mounted widget can make back onto its own pane, and the
 * renderer that mounts it.
 *
 * All four are built to keep one identity for the life of the workspace. They
 * used to close over the active window's tree, so every one of them changed on
 * a window switch — which changed the chat context value, which re-rendered
 * every chat pane in every mounted window. Reading the workspace through a ref
 * costs nothing and makes a switch invisible to the widgets, which is the
 * point: none of them care which window is on screen.
 *
 * Scope is unchanged: `setWidget` is a window-local action, so a pane in a
 * background window is not addressable and these no-op for it exactly as
 * before.
 */

export interface WorkspaceWidgetsDeps {
  readonly state: Workspace;
  readonly dispatch: (action: WorkspaceAction) => void;
  readonly chat: Omit<WorkspaceChatServices, "bindSession" | "setEngine">;
  readonly onError: (message: string) => void;
}

export interface WorkspaceWidgets {
  readonly chatServices: WorkspaceChatServices;
  readonly renderWidget: (
    widget: WidgetState,
    pane: PaneNode,
    focused: boolean,
  ) => React.ReactNode;
}

export function useWorkspaceWidgets(deps: WorkspaceWidgetsDeps): WorkspaceWidgets {
  const { chat, dispatch, onError, state } = deps;

  const latest = useRef({ state, onError });
  latest.current = { state, onError };

  /** The pane, if it is in the active window and holds the kind expected. */
  const activePane = useCallback((paneId: string): PaneNode | null => {
    const { windows, activeWindowId } = latest.current.state;
    const active = windows.find((candidate) => candidate.id === activeWindowId);
    return active ? findPane(active.root, paneId as PaneId) : null;
  }, []);

  /**
   * A chat pane opened as "new session" has no id until its first send creates
   * one. Writing it back here is what makes the pane survive a reload as that
   * conversation rather than as another blank composer.
   */
  const bindSession = useCallback(
    (paneId: string, sessionId: string) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "chat" || pane.widget.sessionId) return;
      dispatch({ type: "setWidget", pane: pane.id, widget: { ...pane.widget, sessionId } });
    },
    [activePane, dispatch],
  );

  /**
   * Give a chat pane an engine of its own. Persisted with the layout, so two
   * panes on two runners survive a reload as two panes on two runners.
   */
  const setEngine = useCallback(
    (paneId: string, engine: PaneEngine) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "chat") return;
      dispatch({ type: "setWidget", pane: pane.id, widget: { ...pane.widget, engine } });
    },
    [activePane, dispatch],
  );

  /**
   * Persist a terminal pane's live PTY ids so its shells survive a reload.
   * Guarded on an actual change: the panel reports on every tab render, and an
   * unguarded dispatch would rewrite the tree — and re-save it — every frame.
   */
  const onPtyIdsChanged = useCallback(
    (paneId: PaneId, ptyIds: readonly string[]) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "terminal") return;
      const current = pane.widget.ptyIds;
      if (current.length === ptyIds.length && current.every((id, i) => id === ptyIds[i])) return;
      dispatch({ type: "setWidget", pane: paneId, widget: { ...pane.widget, ptyIds } });
    },
    [activePane, dispatch],
  );

  const chatServices = useMemo<WorkspaceChatServices>(
    () => ({ ...chat, bindSession, setEngine }),
    [bindSession, chat, setEngine],
  );

  const reportError = useCallback((message: string) => latest.current.onError(message), []);

  const renderWidget = useCallback(
    (widget: WidgetState, pane: PaneNode, focused: boolean) => {
      // Session-bearing widgets keep their session on the widget. Never derive
      // it from the globally focused chat session: panes may be independent.
      return React.createElement(PaneWidget, {
        widget,
        pane,
        focused,
        onError: reportError,
        onPtyIdsChanged,
      });
    },
    [onPtyIdsChanged, reportError],
  );

  return useMemo(() => ({ chatServices, renderWidget }), [chatServices, renderWidget]);
}
