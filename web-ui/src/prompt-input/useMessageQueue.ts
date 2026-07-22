import { useCallback, useEffect, useRef, useState } from "react";
import { fetchQueue, removeQueuedMessage, clearQueue } from "../api";

interface QueueUpdatedDetail {
  sessionID: string;
  pending: string[];
}

/**
 * Follow-up prompts queued (backend-side) while a session's agent is busy. The engine
 * flushes them — joined into one turn — the moment it goes idle. This hook mirrors that
 * queue for the active session: it seeds from the REST endpoint on session change and
 * stays live via the `opman:queue-updated` window event (dispatched from the SSE handler),
 * and exposes optimistic remove/clear actions the backend then confirms over SSE.
 */
export function useMessageQueue(sessionId: string | null) {
  const [queued, setQueued] = useState<string[]>([]);
  // Guard async seeds against a session switch landing out of order.
  const sessionRef = useRef(sessionId);
  sessionRef.current = sessionId;

  useEffect(() => {
    if (!sessionId) {
      setQueued([]);
      return;
    }
    let alive = true;
    fetchQueue(sessionId)
      .then((list) => {
        if (alive && sessionRef.current === sessionId) setQueued(list);
      })
      .catch(() => {
        if (alive && sessionRef.current === sessionId) setQueued([]);
      });
    return () => {
      alive = false;
    };
  }, [sessionId]);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<QueueUpdatedDetail>).detail;
      if (!detail || detail.sessionID !== sessionRef.current) return;
      setQueued(detail.pending ?? []);
    };
    window.addEventListener("opman:queue-updated", handler);
    return () => window.removeEventListener("opman:queue-updated", handler);
  }, []);

  const removeAt = useCallback(
    (index: number) => {
      const sid = sessionRef.current;
      if (!sid) return;
      setQueued((prev) => prev.filter((_, i) => i !== index)); // optimistic
      removeQueuedMessage(sid, index)
        .then((list) => {
          if (sessionRef.current === sid) setQueued(list);
        })
        .catch(() => {
          if (sessionRef.current === sid) fetchQueue(sid).then(setQueued).catch(() => {});
        });
    },
    [],
  );

  const clearAll = useCallback(() => {
    const sid = sessionRef.current;
    if (!sid) return;
    setQueued([]); // optimistic
    clearQueue(sid).catch(() => {
      if (sessionRef.current === sid) fetchQueue(sid).then(setQueued).catch(() => {});
    });
  }, []);

  return { queued, removeAt, clearAll };
}
