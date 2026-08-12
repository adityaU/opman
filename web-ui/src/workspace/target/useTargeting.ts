import { useCallback, useMemo, useRef, useState } from "react";
import type { PaneId, WidgetForPane, WidgetState } from "../types";

/**
 * Pane targeting: "open this — but where?"
 *
 * Arming stashes what is being opened and lets the user answer with one
 * keystroke. It is one mechanism with three entry points — clicking a session
 * in the sidebar, finishing the widget opener, and dragging onto a pane — so
 * that "which pane" is answered the same way every time.
 *
 * The pending widget lives in a ref *and* in state: state so the overlay
 * re-renders, a ref so the command handlers can read it without every one of
 * them having to be re-created when it changes.
 */

export interface TargetRequest {
  readonly widget: WidgetState;
  /** Used by staged widgets whose destination pane is not known while choosing them. */
  readonly widgetForPane?: WidgetForPane;
  /** What the chip names, e.g. a session title. */
  readonly label: string;
}

export interface Targeting {
  readonly active: boolean;
  readonly request: TargetRequest | null;
  readonly arm: (request: TargetRequest) => void;
  readonly cancel: () => void;
  /** Resolve to a pane. Returns the request so the caller can act on it. */
  readonly take: () => TargetRequest | null;
}

export function useTargeting(): Targeting {
  const [request, setRequest] = useState<TargetRequest | null>(null);
  const pending = useRef<TargetRequest | null>(null);

  const arm = useCallback((next: TargetRequest) => {
    pending.current = next;
    setRequest(next);
  }, []);

  const cancel = useCallback(() => {
    pending.current = null;
    setRequest(null);
  }, []);

  const take = useCallback(() => {
    const taken = pending.current;
    pending.current = null;
    setRequest(null);
    return taken;
  }, []);

  return useMemo(
    () => ({ active: request !== null, request, arm, cancel, take }),
    [request, arm, cancel, take],
  );
}

/** Panes as the overlay paints them: ordinal, id, and whether it is focused. */
export interface TargetSlot {
  readonly paneId: PaneId;
  readonly ordinal: number;
  readonly focused: boolean;
}
