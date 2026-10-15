import { useEffect, useRef } from "react";
import { SESSION_HANDOFF, type HandoffEvent } from "../api/session";

/**
 * The shell's (mobile) half of following a conversation opman moved to another runner
 * by itself: when the session on screen is the one that moved, open where it continues.
 * The desktop workspace's panes follow through `useFollowHandoffs`.
 */
export function useShellFollowHandoff(
  activeSessionId: string | null,
  activeProjectIndex: number,
  select: (sessionId: string, projectIndex: number) => void,
): void {
  const latest = useRef({ activeSessionId, activeProjectIndex, select });
  latest.current = { activeSessionId, activeProjectIndex, select };
  useEffect(() => {
    const follow = (event: Event) => {
      const { from, to } = (event as CustomEvent<HandoffEvent>).detail;
      const current = latest.current;
      if (current.activeSessionId === from) current.select(to, current.activeProjectIndex);
    };
    window.addEventListener(SESSION_HANDOFF, follow);
    return () => window.removeEventListener(SESSION_HANDOFF, follow);
  }, []);
}
