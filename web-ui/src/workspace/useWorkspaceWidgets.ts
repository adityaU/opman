import React, { useCallback, useMemo, useRef } from "react";
import { PaneWidget } from "./widgets/PaneWidget";
import { findPane } from "./tree";
import type { WorkspaceAction } from "./reducer";
import type { WorkspaceChatServices } from "./widgets/WorkspaceChatContext";
import type { PaneEngine, PaneId, PaneNode, WidgetState, Workspace } from "./types";

/**
 * The five writes a mounted widget can make back onto its own pane, and the
 * renderer that mounts it.
 *
 * Each one says whether it is navigation. `openWidget` means the pane is now
 * showing a different thing and the pane's trail should record it; `amendWidget`
 * means the pane is showing the same thing and now knows more about it. Getting
 * that wrong is what a single `setWidget` used to hide — a per-pane engine
 * change looked exactly like a place the user had been.
 *
 * All four are built to keep one identity for the life of the workspace. They
 * used to close over the active window's tree, so every one of them changed on
 * a window switch — which changed the chat context value, which re-rendered
 * every chat pane in every mounted window. Reading the workspace through a ref
 * costs nothing and makes a switch invisible to the widgets, which is the
 * point: none of them care which window is on screen.
 *
 * Scope is unchanged: both writes are window-local actions, so a pane in a
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
   *
   * An amend: the pane has been showing this conversation since the user opened
   * it, and the id arriving is the server naming it rather than the user going
   * anywhere. Recording it would put the same conversation in the trail twice —
   * once as "New session" and once as itself.
   */
  const bindSession = useCallback(
    (paneId: string, sessionId: string) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "chat" || pane.widget.sessionId) return;
      dispatch({ type: "amendWidget", pane: pane.id, widget: { ...pane.widget, sessionId } });
    },
    [activePane, dispatch],
  );

  /**
   * Give a chat pane an engine of its own. Persisted with the layout, so two
   * panes on two runners survive a reload as two panes on two runners.
   *
   * An amend: a runner is how the pane talks to the conversation, not which
   * conversation it is on.
   */
  const setEngine = useCallback(
    (paneId: string, engine: PaneEngine) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "chat") return;
      dispatch({ type: "amendWidget", pane: pane.id, widget: { ...pane.widget, engine } });
    },
    [activePane, dispatch],
  );

  /**
   * Remember which shell a terminal pane is showing, so a reload comes back to
   * it. Guarded on an actual change: the panel reports on every settle, and an
   * unguarded dispatch would rewrite the tree — and re-save it — every frame.
   *
   * A record: attaching to a different shell is going somewhere else, and it is
   * the case that used to be lost outright — point the pane at a chat session
   * afterwards and the shell was forgotten, though it was still running.
   */
  const onPtyIdChanged = useCallback(
    (paneId: PaneId, ptyId: string | null) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "terminal" || pane.widget.ptyId === ptyId) return;
      dispatch({ type: "openWidget", pane: paneId, widget: { ...pane.widget, ptyId } });
    },
    [activePane, dispatch],
  );

  /**
   * Remember which file a files pane is on.
   *
   * The widget's `open` was the *request* — what the pane was last asked to
   * reveal — so a file the user reached by clicking around inside the panel was
   * never written down anywhere, and "the file I had open before this one" had
   * no answer. The panel now reports its own cursor, which makes the two agree.
   *
   * The path is made absolute first. The panel works in paths relative to the
   * project root while a reveal request arrives absolute, so the same file
   * reached the two ways would be two entries in the trail — showing the same
   * name twice, and neither of them collapsing into the other.
   *
   * `seq` is carried forward rather than minted: this is the panel telling us
   * where it already is, so re-arming it would ask it to reveal a file it is
   * already displaying.
   */
  const onActiveFileChanged = useCallback(
    (paneId: PaneId, path: string | null) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "files" || !path) return;
      const absolute = path.startsWith("/") ? path : `${pane.widget.projectPath}/${path}`;
      if (pane.widget.open?.path === absolute) return;
      const open = { path: absolute, line: null, seq: pane.widget.open?.seq ?? 0 };
      dispatch({ type: "openWidget", pane: paneId, widget: { ...pane.widget, open } });
    },
    [activePane, dispatch],
  );

  const chatServices = useMemo<WorkspaceChatServices>(
    () => ({ ...chat, bindSession, setEngine }),
    [bindSession, chat, setEngine],
  );

  /**
   * Remember where a browser pane is, so a reload comes back to the page rather
   * than to a blank tab. Guarded like the PTY ids above: the panel reports the
   * URL on every settled navigation, including ones that did not move.
   */
  const onBrowserUrlChanged = useCallback(
    (paneId: PaneId, url: string) => {
      const pane = activePane(paneId);
      if (pane?.widget?.kind !== "browser" || pane.widget.url === url) return;
      dispatch({ type: "openWidget", pane: paneId, widget: { ...pane.widget, url } });
    },
    [activePane, dispatch],
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
        onPtyIdChanged,
        onBrowserUrlChanged,
        onActiveFileChanged,
      });
    },
    [onActiveFileChanged, onBrowserUrlChanged, onPtyIdChanged, reportError],
  );

  return useMemo(() => ({ chatServices, renderWidget }), [chatServices, renderWidget]);
}
