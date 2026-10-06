/**
 * Placing a widget that arrived whole — from an embedded pane asking its parent
 * to open something, rather than from this instance's own tool cards.
 *
 * The embed knows *what* (it built the widget against its own server's
 * projects) and nothing about *where*: it is one pane and cannot see the
 * others. So this is the same question `planFileOpen` and `planBrowserOpen`
 * answer, routed to them for the two kinds they own, and answered by the same
 * rule for the rest: reuse a pane already on that target, else fill an empty
 * focused pane, else a column beside the focused one.
 */

import { SERVER_ID } from "../api/base";
import { planBrowserOpen } from "./browserOpen";
import { planFileOpenIn, type FileOpenPlan } from "./fileOpen";
import { sameTarget } from "./history";
import type { PaneId, PaneNode, WidgetState } from "./types";

export type ForeignOpenPlan = FileOpenPlan;

/**
 * Tag a widget with the server it came from — or untag it, when that server is
 * this instance's own, so "absent means own" stays the only spelling of it.
 */
export function withServer(widget: WidgetState, server: string, own: string = SERVER_ID): WidgetState {
  const { server: _previous, ...rest } = widget;
  return server && server !== own ? { ...rest, server } : rest;
}

/** Whether a widget belongs to a server other than this instance's. */
export function isForeign(widget: WidgetState | null | undefined, own: string = SERVER_ID): boolean {
  return Boolean(widget?.server && widget.server !== own);
}

/**
 * `seq` re-arms a reveal: the embed minted its own, which means nothing to the
 * pane that will show it here.
 */
export function planForeignOpen(
  widget: WidgetState,
  panes: readonly PaneNode[],
  focusedPaneId: PaneId,
  seq: number,
): ForeignOpenPlan {
  if (widget.kind === "files" && widget.open) {
    const open = { ...widget.open, seq };
    return planFileOpenIn(widget.projectPath, widget.server, open, panes, focusedPaneId);
  }
  if (widget.kind === "browser" && widget.url) {
    return planBrowserOpen(widget.projectPath, widget.url, panes, focusedPaneId, widget.server);
  }

  // A chat with no session, or a terminal with no shell, is a request for a
  // *new* one — never "the pane already showing a blank one".
  const reusable = !(widget.kind === "chat" && !widget.sessionId)
    && !(widget.kind === "terminal" && !widget.ptyId);
  const existing = reusable ? panes.find((pane) => sameTarget(pane.widget, widget)) : undefined;
  if (existing?.widget) return { action: "place", pane: existing.id, widget: existing.widget };

  const focused = panes.find((pane) => pane.id === focusedPaneId);
  if (focused && !focused.widget) return { action: "place", pane: focused.id, widget };
  return { action: "split", pane: focusedPaneId, widget };
}
