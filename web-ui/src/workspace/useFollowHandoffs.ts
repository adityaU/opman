import { useEffect } from "react";
import { SESSION_HANDOFF, type HandoffEvent } from "../api/session";
import type { WorkspaceAction } from "./reducer";

/**
 * Keep every chat pane on a conversation that opman moved to another runner by itself.
 *
 * A send the user made reports its own handoff (see `readHandoff`); this covers the moves
 * nobody in the browser asked for — a turn that ran out of usage while the session was
 * left working, continued on the next runner of the user's list.
 */
export function useFollowHandoffs(dispatch: (action: WorkspaceAction) => void): void {
  useEffect(() => {
    const follow = (event: Event) => {
      const { from, to } = (event as CustomEvent<HandoffEvent>).detail;
      dispatch({ type: "followSession", from, to });
    };
    window.addEventListener(SESSION_HANDOFF, follow);
    return () => window.removeEventListener(SESSION_HANDOFF, follow);
  }, [dispatch]);
}
