import { useEffect, useRef, useCallback, useState } from "react";
import {
  createEventsSSE,
  createSessionEventsSSE,
  parseOpenCodeEvent,
  fetchAppState,
  fetchSessionMessages,
  fetchSessionStats,
  fetchTheme,
  type AppState,
  type SessionStats,
  type ThemeColors,
} from "../api";
import type { Message, PermissionRequest, QuestionRequest, OpenCodeEvent } from "../types";

/** Number of messages to fetch per page */
const PAGE_SIZE = 100;

/** Apply ThemeColors to CSS custom properties */
function applyThemeToCss(colors: ThemeColors) {
  const root = document.documentElement.style;
  root.setProperty("--color-primary", colors.primary);
  root.setProperty("--color-secondary", colors.secondary);
  root.setProperty("--color-accent", colors.accent);
  root.setProperty("--color-bg", colors.background);
  root.setProperty("--color-bg-panel", colors.background_panel);
  root.setProperty("--color-bg-element", colors.background_element);
  root.setProperty("--color-text", colors.text);
  root.setProperty("--color-text-muted", colors.text_muted);
  root.setProperty("--color-border", colors.border);
  root.setProperty("--color-border-active", colors.border_active);
  root.setProperty("--color-border-subtle", colors.border_subtle);
  root.setProperty("--color-error", colors.error);
  root.setProperty("--color-warning", colors.warning);
  root.setProperty("--color-success", colors.success);
  root.setProperty("--color-info", colors.info);
}

export interface SSEState {
  appState: AppState | null;
  messages: Message[];
  stats: SessionStats | null;
  busySessions: Set<string>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  sessionStatus: "idle" | "busy";
  refreshState: () => Promise<void>;
  refreshMessages: () => Promise<void>;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  /** Load older messages (infinite scroll upward). No-op if already at beginning. */
  loadMoreMessages: () => Promise<void>;
  /** Whether there are older messages that haven't been loaded yet. */
  hasMoreMessages: boolean;
  /** Whether a "load more" request is currently in flight. */
  isLoadingMore: boolean;
}

export function useSSE(): SSEState {
  const [appState, setAppState] = useState<AppState | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [stats, setStats] = useState<SessionStats | null>(null);
  const [busySessions, setBusySessions] = useState<Set<string>>(new Set());
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [sessionStatus, setSessionStatus] = useState<"idle" | "busy">("idle");
  const [hasMoreMessages, setHasMoreMessages] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval>>();
  const activeSessionRef = useRef<string | null>(null);

  /**
   * Track how many messages from the *start* of the list we have loaded.
   * This is the offset where the currently displayed messages begin.
   * E.g., if total=573 and we loaded the last 100, loadedOffset=473.
   */
  const loadedOffsetRef = useRef(0);

  /** Total messages available on the server for the current session. */
  const totalRef = useRef(0);

  /**
   * Generation counter — incremented every time the active session changes.
   * Used to discard stale fetch results from a previous session.
   */
  const sessionGenRef = useRef(0);

  const refreshState = useCallback(async () => {
    try {
      const s = await fetchAppState();
      setAppState(s);
      const busy = new Set<string>();
      for (const p of s.projects) {
        for (const sid of p.busy_sessions) busy.add(sid);
      }
      setBusySessions(busy);
      const proj = s.projects[s.active_project];
      if (proj?.active_session) {
        const st = await fetchSessionStats(proj.active_session);
        setStats(st);
      }
    } catch (e) {
      console.error("Failed to fetch state:", e);
    }
  }, []);

  /** Fetch the last PAGE_SIZE messages for the active session (initial load / refresh). */
  const refreshMessages = useCallback(async () => {
    const sid = activeSessionRef.current;
    if (!sid) return;
    const gen = sessionGenRef.current;
    try {
      // Single request: use `tail` to fetch the last PAGE_SIZE messages.
      const { messages: tailMsgs, total, offset } = await fetchSessionMessages(sid, {
        tail: PAGE_SIZE,
      });
      // Guard: if session changed while we were fetching, discard result.
      if (gen !== sessionGenRef.current) return;
      totalRef.current = total;
      loadedOffsetRef.current = offset;
      setMessages(tailMsgs);
      setHasMoreMessages(offset > 0);
    } catch (e) {
      if (gen !== sessionGenRef.current) return;
      console.error("Failed to fetch messages:", e);
      setMessages([]);
      loadedOffsetRef.current = 0;
      totalRef.current = 0;
      setHasMoreMessages(false);
    }
  }, []);

  /** Load older messages (prepend to the current list). */
  const loadMoreMessages = useCallback(async () => {
    const sid = activeSessionRef.current;
    if (!sid || isLoadingMore || loadedOffsetRef.current <= 0) return;
    const gen = sessionGenRef.current;
    setIsLoadingMore(true);
    try {
      const newOffset = Math.max(0, loadedOffsetRef.current - PAGE_SIZE);
      const limit = loadedOffsetRef.current - newOffset; // usually PAGE_SIZE, but could be less at the start
      const { messages: olderMsgs } = await fetchSessionMessages(sid, {
        offset: newOffset,
        limit,
      });
      if (gen !== sessionGenRef.current) return;
      loadedOffsetRef.current = newOffset;
      setHasMoreMessages(newOffset > 0);
      setMessages((prev) => [...olderMsgs, ...prev]);
    } catch (e) {
      console.error("Failed to load more messages:", e);
    } finally {
      setIsLoadingMore(false);
    }
  }, [isLoadingMore]);

  const clearPermission = useCallback((id: string) => {
    setPermissions((prev) => prev.filter((p) => p.id !== id));
  }, []);

  const clearQuestion = useCallback((id: string) => {
    setQuestions((prev) => prev.filter((q) => q.id !== id));
  }, []);

  // Track active session changes and fetch messages
  useEffect(() => {
    if (!appState) return;
    const proj = appState.projects[appState.active_project];
    const sid = proj?.active_session ?? null;
    if (sid !== activeSessionRef.current) {
      // Session changed — bump generation to invalidate any in-flight fetches
      sessionGenRef.current += 1;
      const gen = sessionGenRef.current;
      activeSessionRef.current = sid;

      if (sid) {
        // Clear old messages immediately so the UI doesn't show stale data
        setMessages([]);
        setHasMoreMessages(false);
        loadedOffsetRef.current = 0;
        totalRef.current = 0;

        // Single fetch: get the last PAGE_SIZE messages using `tail`.
        fetchSessionMessages(sid, { tail: PAGE_SIZE })
          .then(({ messages: tailMsgs, total, offset }) => {
            // Guard: discard if session changed during fetch
            if (gen !== sessionGenRef.current) return;
            totalRef.current = total;
            loadedOffsetRef.current = offset;
            setMessages(tailMsgs);
            setHasMoreMessages(offset > 0);
          })
          .catch(() => {
            if (gen !== sessionGenRef.current) return;
            setMessages([]);
            loadedOffsetRef.current = 0;
            totalRef.current = 0;
            setHasMoreMessages(false);
          });
      } else {
        setMessages([]);
        loadedOffsetRef.current = 0;
        totalRef.current = 0;
        setHasMoreMessages(false);
      }
    }
  }, [appState]);

  // SSE connections
  useEffect(() => {
    refreshState();
    fetchTheme().then((colors) => {
      if (colors) applyThemeToCss(colors);
    });

    pollRef.current = setInterval(refreshState, 5000);

    // App events SSE (state changes, busy/idle, stats, theme)
    const appSSE = createEventsSSE();
    appSSE.addEventListener("state_changed", () => refreshState());
    appSSE.addEventListener("session_busy", (e: MessageEvent) => {
      setBusySessions((prev) => new Set([...prev, e.data]));
      if (e.data === activeSessionRef.current) setSessionStatus("busy");
    });
    appSSE.addEventListener("session_idle", (e: MessageEvent) => {
      setBusySessions((prev) => {
        const next = new Set(prev);
        next.delete(e.data);
        return next;
      });
      if (e.data === activeSessionRef.current) setSessionStatus("idle");
    });
    appSSE.addEventListener("stats_updated", (e: MessageEvent) => {
      try {
        setStats(JSON.parse(e.data));
      } catch { /* ignore */ }
    });
    appSSE.addEventListener("theme_changed", (e: MessageEvent) => {
      try {
        applyThemeToCss(JSON.parse(e.data));
      } catch { /* ignore */ }
    });

    // Session events SSE (proxied from opencode server)
    const sessionSSE = createSessionEventsSSE();
    sessionSSE.addEventListener("opencode", (e: MessageEvent) => {
      const event = parseOpenCodeEvent(e.data);
      if (!event) return;
      handleOpenCodeEvent(event);
    });

    function handleOpenCodeEvent(event: OpenCodeEvent) {
      const props = event.properties || {};
      switch (event.type) {
        case "message.updated": {
          // Only refresh messages if the update is for the currently active session
          const msgSessionId = (props.info as Record<string, unknown> | undefined)?.sessionID as string | undefined;
          if (activeSessionRef.current && (!msgSessionId || msgSessionId === activeSessionRef.current)) {
            const sid = activeSessionRef.current;
            const gen = sessionGenRef.current;

            // Re-fetch from our current loaded offset to get updated content + new messages.
            // Save the OLD total before updating so we can correctly compute prepended counts.
            const oldTotal = totalRef.current;
            const currentOffset = loadedOffsetRef.current;

            fetchSessionMessages(sid, { offset: currentOffset })
              .then(({ messages: freshMsgs, total }) => {
                // Guard: discard if session changed
                if (gen !== sessionGenRef.current) return;
                if (sid !== activeSessionRef.current) return;

                totalRef.current = total;
                setMessages((prev) => {
                  if (currentOffset > 0 && prev.length > 0) {
                    // We may have older messages that were prepended via infinite scroll.
                    // freshMsgs covers [currentOffset .. end].
                    // The number of messages in the "tail" window BEFORE this refresh
                    // is (oldTotal - currentOffset). Any extra messages in `prev` beyond
                    // that are prepended older messages we need to keep.
                    const prevTailSize = Math.max(0, oldTotal - currentOffset);
                    const prependedCount = Math.max(0, prev.length - prevTailSize);
                    if (prependedCount > 0) {
                      const olderMsgs = prev.slice(0, prependedCount);
                      return [...olderMsgs, ...freshMsgs];
                    }
                  }
                  return freshMsgs;
                });
              })
              .catch(() => {
                // Silently ignore — next SSE event will retry
              });
          }
          break;
        }
        case "session.status": {
          const sid = props.sessionID as string | undefined;
          // The upstream opencode server sends status as a nested object:
          // { type: "session.status", properties: { sessionID: "...", status: { type: "busy" } } }
          const rawStatus = props.status;
          let statusStr: string | undefined;
          if (typeof rawStatus === "string") {
            statusStr = rawStatus;
          } else if (rawStatus && typeof rawStatus === "object") {
            statusStr = (rawStatus as Record<string, unknown>).type as string | undefined;
          }
          if (sid === activeSessionRef.current) {
            setSessionStatus(statusStr === "busy" || statusStr === "retry" ? "busy" : "idle");
          }
          break;
        }
        case "session.created":
        case "session.updated":
        case "session.deleted":
          refreshState();
          break;
        case "permission.asked": {
          const perm: PermissionRequest = {
            id: (props.id ?? props.requestID ?? "") as string,
            sessionID: (props.sessionID ?? "") as string,
            toolName: (props.toolName ?? "") as string,
            description: props.description as string | undefined,
            args: props.args as Record<string, unknown> | undefined,
            time: Date.now(),
          };
          if (perm.id && perm.sessionID === activeSessionRef.current) {
            setPermissions((prev) => [...prev.filter((p) => p.id !== perm.id), perm]);
          }
          break;
        }
        case "question.asked": {
          const q: QuestionRequest = {
            id: (props.id ?? props.requestID ?? "") as string,
            sessionID: (props.sessionID ?? "") as string,
            title: (props.title ?? "") as string,
            questions: (props.questions ?? []) as QuestionRequest["questions"],
            time: Date.now(),
          };
          if (q.id && q.sessionID === activeSessionRef.current) {
            setQuestions((prev) => [...prev.filter((qp) => qp.id !== q.id), q]);
          }
          break;
        }
        case "todo.updated":
          // Could refresh todos here
          break;
      }
    }

    return () => {
      clearInterval(pollRef.current);
      appSSE.close();
      sessionSSE.close();
    };
  }, [refreshState]);

  return {
    appState,
    messages,
    stats,
    busySessions,
    permissions,
    questions,
    sessionStatus,
    refreshState,
    refreshMessages,
    clearPermission,
    clearQuestion,
    loadMoreMessages,
    hasMoreMessages,
    isLoadingMore,
  };
}
