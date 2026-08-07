import { useCallback, useSyncExternalStore } from "react";
import { EMPTY_VIEW, getSessionView, subscribeSession, type SessionView } from "./sessionStore";

/**
 * Read one session's live transcript, wherever it is on screen.
 *
 * `useSyncExternalStore` rather than a context: a chat pane must re-render when
 * *its* session streams a token and stay still when any other one does, and a
 * context value would wake every consumer on every event in the workspace.
 */
export function useSessionView(sessionId: string | null): SessionView {
  const subscribe = useCallback(
    (listener: () => void) => {
      if (!sessionId) return () => {};
      return subscribeSession(sessionId, listener);
    },
    [sessionId],
  );

  const snapshot = useCallback(
    () => (sessionId ? getSessionView(sessionId) : EMPTY_VIEW),
    [sessionId],
  );

  // Server snapshot is the same: there is no SSR here, and returning a
  // distinct object would trip the hydration mismatch warning.
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}
