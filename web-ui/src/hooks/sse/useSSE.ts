import { useEffect, useRef, useCallback, useState } from "react";
import {
  createEventsSSE,
  createSessionEventsSSE,
  parseOpenCodeEvent,
  fetchAppState,
  fetchSessionMessages,
  fetchSessionStats,
  fetchTheme,
  fetchPending,
  type AppState,
  type SessionStats,
  type ActivityEvent,
  type ClientPresence,
} from "../../api";
import type { Message, PermissionRequest, QuestionRequest } from "../../types";
import { applyThemeToCss } from "../../utils/theme";

import type { SSEState, WatcherStatus } from "./types";
import { type MessageMap, mapToSortedArray, getMessageTime } from "./messageMap";
import { handleOpenCodeEvent, setupAppSSEListeners } from "./eventHandler";
import { formatPermissionDescription, deriveQuestionTitle, transformQuestionInfo } from "./transforms";

/** Number of messages to load per page. */
const MESSAGE_PAGE_SIZE = 50;

/** Maximum number of sessions to keep cached in memory. */
const MAX_SESSION_CACHE = 20;

/** Cached state for a previously-visited session. */
interface CachedSession {
  messageMap: MessageMap;
  subagentMaps: Map<string, MessageMap>;
  stats: SessionStats | null;
  hasOlderMessages: boolean;
  totalMessageCount: number;
  liveActivityEvents: ActivityEvent[];
  /** Timestamp of last access — used for LRU eviction. */
  lastAccess: number;
}

export function useSSE(): SSEState {
  const [appState, setAppState] = useState<AppState | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [stats, setStats] = useState<SessionStats | null>(null);
  const [busySessions, setBusySessions] = useState<Set<string>>(new Set());
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [sessionStatus, setSessionStatus] = useState<"idle" | "busy">("idle");
  const [isLoadingMessages, setIsLoadingMessages] = useState(false);
  const [isLoadingOlder, setIsLoadingOlder] = useState(false);
  const [hasOlderMessages, setHasOlderMessages] = useState(false);
  const [totalMessageCount, setTotalMessageCount] = useState(0);
  const [watcherStatus, setWatcherStatus] = useState<WatcherStatus | null>(null);
  const [subagentMessages, setSubagentMessages] = useState<Map<string, Message[]>>(new Map());
  const [fileEditCount, setFileEditCount] = useState(0);
  const [mcpEditorOpenPath, setMcpEditorOpenPath] = useState<string | null>(null);
  const [mcpEditorOpenLine, setMcpEditorOpenLine] = useState<number | null>(null);
  const [mcpTerminalFocusId, setMcpTerminalFocusId] = useState<string | null>(null);
  const [mcpAgentActivity, setMcpAgentActivity] = useState<Map<string, boolean>>(new Map());
  const [presenceClients, setPresenceClients] = useState<ClientPresence[]>([]);
  const [liveActivityEvents, setLiveActivityEvents] = useState<ActivityEvent[]>([]);
  const [crossSessionPermissions, setCrossSessionPermissions] = useState<PermissionRequest[]>([]);
  const [crossSessionQuestions, setCrossSessionQuestions] = useState<QuestionRequest[]>([]);
  const activeSessionRef = useRef<string | null>(null);
  const appliedTitleRef = useRef<string | null>(null);

  const sessionGenRef = useRef(0);
  const messageMapRef = useRef<MessageMap>(new Map());
  const subagentMapsRef = useRef<Map<string, MessageMap>>(new Map());

  /** LRU session cache — keeps previously-visited sessions in memory for instant switching. */
  const sessionCacheRef = useRef<Map<string, CachedSession>>(new Map());

  /** Mirror of React state values needed by the cache save function (refs can be read synchronously). */
  const statsRef = useRef<SessionStats | null>(null);
  const hasOlderRef = useRef(false);
  const totalCountRef = useRef(0);
  const liveActivityRef = useRef<ActivityEvent[]>([]);

  // Keep cache-related refs in sync with React state
  useEffect(() => { statsRef.current = stats; }, [stats]);
  useEffect(() => { hasOlderRef.current = hasOlderMessages; }, [hasOlderMessages]);
  useEffect(() => { totalCountRef.current = totalMessageCount; }, [totalMessageCount]);
  useEffect(() => { liveActivityRef.current = liveActivityEvents; }, [liveActivityEvents]);

  // ── Session cache helpers ──────────────────────────────────────
  /** Save the current active session's state into the cache. */
  const saveCurrentSessionToCache = useCallback(() => {
    const sid = activeSessionRef.current;
    if (!sid) return;
    const cache = sessionCacheRef.current;
    cache.set(sid, {
      messageMap: messageMapRef.current,
      subagentMaps: subagentMapsRef.current,
      stats: statsRef.current,
      hasOlderMessages: hasOlderRef.current,
      totalMessageCount: totalCountRef.current,
      liveActivityEvents: liveActivityRef.current,
      lastAccess: Date.now(),
    });
    // LRU eviction
    if (cache.size > MAX_SESSION_CACHE) {
      let oldestKey: string | null = null;
      let oldestTime = Infinity;
      for (const [key, entry] of cache) {
        if (entry.lastAccess < oldestTime) {
          oldestTime = entry.lastAccess;
          oldestKey = key;
        }
      }
      if (oldestKey) cache.delete(oldestKey);
    }
  }, []);

  /** Restore a session from the cache if available. Returns true if restored. */
  const restoreSessionFromCache = useCallback((sid: string): boolean => {
    const cached = sessionCacheRef.current.get(sid);
    if (!cached) return false;
    cached.lastAccess = Date.now();
    messageMapRef.current = cached.messageMap;
    subagentMapsRef.current = cached.subagentMaps;
    setMessages(mapToSortedArray(cached.messageMap));
    setStats(cached.stats);
    setHasOlderMessages(cached.hasOlderMessages);
    setTotalMessageCount(cached.totalMessageCount);
    setLiveActivityEvents(cached.liveActivityEvents);
    // Flush subagent messages
    const result = new Map<string, Message[]>();
    for (const [subSid, map] of cached.subagentMaps) {
      result.set(subSid, mapToSortedArray(map));
    }
    setSubagentMessages(result);
    return true;
  }, []);

  // ── Flush helpers (debounced to ~1 frame) ─────────────────────
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushMessages = useCallback(() => {
    if (flushTimerRef.current) return;
    flushTimerRef.current = setTimeout(() => {
      flushTimerRef.current = null;
      setMessages(mapToSortedArray(messageMapRef.current));
    }, 16);
  }, []);

  const flushSubagentTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushSubagentMessages = useCallback(() => {
    if (flushSubagentTimerRef.current) return;
    flushSubagentTimerRef.current = setTimeout(() => {
      flushSubagentTimerRef.current = null;
      const result = new Map<string, Message[]>();
      for (const [sid, map] of subagentMapsRef.current) {
        result.set(sid, mapToSortedArray(map));
      }
      setSubagentMessages(result);
    }, 16);
  }, []);

  // ── Data fetchers ─────────────────────────────────────────────
  const refreshState = useCallback(async () => {
    try {
      const s = await fetchAppState();
      setAppState(s);
      // Set page title from instance name (tunnel subdomain) if provided — only when changed
      if (s.instance_name && s.instance_name !== appliedTitleRef.current) {
        document.title = s.instance_name;
        appliedTitleRef.current = s.instance_name;
      }
      setBusySessions((prev) => {
        const next = new Set(prev);
        for (const p of s.projects) {
          for (const sid of p.busy_sessions) next.add(sid);
        }
        return next;
      });
    } catch (e) {
      console.error("Failed to fetch state:", e);
    }
  }, []);

  /** Hydrate pending permissions/questions from server-side tracking (survives reload). */
  const hydratePending = useCallback(async () => {
    try {
      const pending = await fetchPending();
      const activeSid = activeSessionRef.current;
      const perms: PermissionRequest[] = [];
      const crossPerms: PermissionRequest[] = [];
      for (const raw of pending.permissions) {
        const props = raw as Record<string, unknown>;
        const perm: PermissionRequest = {
          id: (props.id ?? props.requestID ?? "") as string,
          sessionID: (props.sessionID ?? "") as string,
          toolName: (props.permission ?? props.toolName ?? "") as string,
          description: formatPermissionDescription(props),
          patterns: Array.isArray(props.patterns) ? (props.patterns as string[]) : undefined,
          metadata: (props.metadata && typeof props.metadata === "object")
            ? props.metadata as Record<string, unknown> : undefined,
          time: typeof props.time === "number" ? props.time : Date.now(),
        };
        if (!perm.id) continue;
        if (activeSid && perm.sessionID === activeSid) {
          perms.push(perm);
        } else {
          crossPerms.push(perm);
        }
      }
      const qs: QuestionRequest[] = [];
      const crossQs: QuestionRequest[] = [];
      for (const raw of pending.questions) {
        const props = raw as Record<string, unknown>;
        const rawQuestions = Array.isArray(props.questions) ? props.questions : [];
        const q: QuestionRequest = {
          id: (props.id ?? props.requestID ?? "") as string,
          sessionID: (props.sessionID ?? "") as string,
          title: deriveQuestionTitle(props, rawQuestions),
          questions: rawQuestions.map(transformQuestionInfo),
          time: typeof props.time === "number" ? props.time : Date.now(),
        };
        if (!q.id) continue;
        if (activeSid && q.sessionID === activeSid) {
          qs.push(q);
        } else {
          crossQs.push(q);
        }
      }
      // Merge with existing state (SSE may have already delivered some)
      setPermissions((prev) => {
        const ids = new Set(prev.map((p) => p.id));
        return [...prev, ...perms.filter((p) => !ids.has(p.id))];
      });
      setCrossSessionPermissions((prev) => {
        const ids = new Set(prev.map((p) => p.id));
        return [...prev, ...crossPerms.filter((p) => !ids.has(p.id))];
      });
      setQuestions((prev) => {
        const ids = new Set(prev.map((q) => q.id));
        return [...prev, ...qs.filter((q) => !ids.has(q.id))];
      });
      setCrossSessionQuestions((prev) => {
        const ids = new Set(prev.map((q) => q.id));
        return [...prev, ...crossQs.filter((q) => !ids.has(q.id))];
      });
    } catch (e) {
      console.error("hydratePending failed:", e);
    }
  }, []);

  const refreshMessages = useCallback(async () => {
    const sid = activeSessionRef.current;
    if (!sid) return;
    const gen = sessionGenRef.current;
    try {
      const resp = await fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE });
      if (gen !== sessionGenRef.current) return;
      const map = messageMapRef.current;
      for (const msg of resp.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (!id) continue;
        const existing = map.get(id);
        if (!existing) {
          map.set(id, msg);
        } else {
          map.set(id, {
            ...existing,
            info: { ...existing.info, ...msg.info },
            metadata: msg.metadata ?? existing.metadata,
            parts: existing.parts.length > 0 ? existing.parts : msg.parts,
          });
        }
      }
      setMessages(mapToSortedArray(map));
      setHasOlderMessages(resp.has_more);
      setTotalMessageCount(resp.total);
    } catch (e) {
      console.error("refreshMessages failed:", e);
    }
  }, []);

  const loadOlderMessages = useCallback(async (): Promise<boolean> => {
    const sid = activeSessionRef.current;
    if (!sid || isLoadingOlder) return false;
    const map = messageMapRef.current;
    let oldestTs = Infinity;
    for (const msg of map.values()) {
      const ts = getMessageTime(msg);
      if (ts > 0 && ts < oldestTs) oldestTs = ts;
    }
    if (oldestTs === Infinity) return false;
    setIsLoadingOlder(true);
    try {
      const gen = sessionGenRef.current;
      const resp = await fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE, before: oldestTs });
      if (gen !== sessionGenRef.current) return false;
      for (const msg of resp.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id && !map.has(id)) map.set(id, msg);
      }
      setMessages(mapToSortedArray(map));
      setHasOlderMessages(resp.has_more);
      return resp.has_more;
    } catch { return false; }
    finally { setIsLoadingOlder(false); }
  }, [isLoadingOlder]);

  // ── Simple callbacks ──────────────────────────────────────────
  const clearPermission = useCallback((id: string) => {
    setPermissions((prev) => prev.filter((p) => p.id !== id));
  }, []);
  const clearQuestion = useCallback((id: string) => {
    setQuestions((prev) => prev.filter((q) => q.id !== id));
  }, []);
  const clearMcpEditorOpen = useCallback(() => {
    setMcpEditorOpenPath(null); setMcpEditorOpenLine(null);
  }, []);
  const clearMcpTerminalFocus = useCallback(() => { setMcpTerminalFocusId(null); }, []);

  const addOptimisticMessage = useCallback((text: string) => {
    const id = `__optimistic__${Date.now()}`;
    const msg: Message = {
      info: { role: "user", messageID: id, id, sessionID: activeSessionRef.current ?? undefined, time: Date.now() / 1000 },
      parts: [{ type: "text", text }],
    };
    messageMapRef.current.set(id, msg);
    flushMessages();
  }, [flushMessages]);

  // ── Track active session changes ──────────────────────────────
  useEffect(() => {
    if (!appState) return;
    const proj = appState.projects[appState.active_project];
    const sid = proj?.active_session ?? null;
    if (sid !== activeSessionRef.current) {
      // Save current session to cache before switching away
      saveCurrentSessionToCache();

      sessionGenRef.current += 1;
      const gen = sessionGenRef.current;
      activeSessionRef.current = sid;

      if (sid) {
        // Try to restore from cache (instant switch)
        const restored = restoreSessionFromCache(sid);
        if (restored) {
          // Cache hit — show cached data immediately, then background-refresh
          // to pick up any messages that arrived while this session was inactive
          setIsLoadingMessages(false);
          fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE })
            .then((resp) => {
              if (gen !== sessionGenRef.current) return;
              const map = messageMapRef.current;
              let changed = false;
              for (const msg of resp.messages) {
                const id = msg.info.messageID || msg.info.id || "";
                if (!id) continue;
                const existing = map.get(id);
                if (!existing) {
                  map.set(id, msg);
                  changed = true;
                } else {
                  // Merge updated info/parts (same logic as refreshMessages)
                  map.set(id, {
                    ...existing,
                    info: { ...existing.info, ...msg.info },
                    metadata: msg.metadata ?? existing.metadata,
                    parts: existing.parts.length > 0 ? existing.parts : msg.parts,
                  });
                  changed = true;
                }
              }
              if (changed) setMessages(mapToSortedArray(map));
              setHasOlderMessages(resp.has_more);
              setTotalMessageCount(resp.total);
            })
            .catch(() => {});
          fetchSessionStats(sid).then((st) => { if (gen !== sessionGenRef.current) return; setStats(st); }).catch(() => {});
        } else {
          // Cache miss — fresh fetch with loading indicator
          messageMapRef.current = new Map();
          subagentMapsRef.current = new Map();
          setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0); setLiveActivityEvents([]);
          setSubagentMessages(new Map());
          setIsLoadingMessages(true);
          fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE })
            .then((resp) => {
              if (gen !== sessionGenRef.current) return;
              const newMap: MessageMap = new Map();
              for (const msg of resp.messages) { const id = msg.info.messageID || msg.info.id || ""; if (id) newMap.set(id, msg); }
              messageMapRef.current = newMap;
              setMessages(mapToSortedArray(newMap)); setHasOlderMessages(resp.has_more); setTotalMessageCount(resp.total);
            })
            .catch(() => { if (gen !== sessionGenRef.current) return; setMessages([]); })
            .finally(() => { if (gen !== sessionGenRef.current) return; setIsLoadingMessages(false); });
          fetchSessionStats(sid).then((st) => { if (gen !== sessionGenRef.current) return; setStats(st); }).catch(() => {});
        }
        // Hydrate pending permissions/questions from server-side tracking
        hydratePending();
      } else {
        messageMapRef.current = new Map();
        subagentMapsRef.current = new Map();
        setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0); setLiveActivityEvents([]);
        setSubagentMessages(new Map());
        setIsLoadingMessages(false);
      }
    }
  }, [appState, saveCurrentSessionToCache, restoreSessionFromCache]);

  // ── SSE connections (set up once on mount) ────────────────────
  useEffect(() => {
    refreshState();
    fetchTheme().then((colors) => { if (colors) applyThemeToCss(colors); });

    let lastEventTime = Date.now();
    let sessionSseNeedsRecovery = false;
    const touchEvent = () => { lastEventTime = Date.now(); };
    const recoverAfterReconnect = () => {
      console.info("[SSE] Recovering after reconnection");
      refreshState(); refreshMessages(); hydratePending();
    };

    // App SSE
    const appSSE = createEventsSSE();
    setupAppSSEListeners(appSSE, {
      activeSessionRef, refreshState, touchEvent, recoverAfterReconnect,
      setBusySessions, setSessionStatus, setStats, setWatcherStatus,
      setMcpEditorOpenPath, setMcpEditorOpenLine, setMcpTerminalFocusId,
      setMcpAgentActivity, setPresenceClients, setLiveActivityEvents,
    });

    // Session SSE
    const sessionSSE = createSessionEventsSSE();
    sessionSSE.addEventListener("heartbeat", () => { touchEvent(); });
    sessionSSE.addEventListener("lagged", () => { console.warn("[SSE] Session events lagged"); recoverAfterReconnect(); });
    sessionSSE.onerror = () => { console.warn("[SSE] Session events connection error"); sessionSseNeedsRecovery = true; };
    sessionSSE.onopen = () => { touchEvent(); if (sessionSseNeedsRecovery) { sessionSseNeedsRecovery = false; recoverAfterReconnect(); } };
    sessionSSE.addEventListener("opencode", (e: MessageEvent) => {
      touchEvent();
      const event = parseOpenCodeEvent(e.data);
      if (!event) return;
      handleOpenCodeEvent(
        { activeSessionRef, messageMapRef, subagentMapsRef, sessionCacheRef,
          flushMessages, flushSubagentMessages,
          refreshState, setStats, setSessionStatus, setPermissions, setQuestions,
          setCrossSessionPermissions, setCrossSessionQuestions, setFileEditCount },
        event,
      );
    });

    // Stale-connection watchdog
    const STALE_THRESHOLD_MS = 45_000;
    const watchdogInterval = setInterval(() => {
      const elapsed = Date.now() - lastEventTime;
      if (elapsed > STALE_THRESHOLD_MS) {
        console.warn(`[SSE] No events in ${Math.round(elapsed / 1000)}s — forcing reconnect`);
        recoverAfterReconnect(); lastEventTime = Date.now();
      }
    }, 10_000);

    return () => {
      appSSE.close(); sessionSSE.close(); clearInterval(watchdogInterval);
      if (flushTimerRef.current) { clearTimeout(flushTimerRef.current); flushTimerRef.current = null; }
      if (flushSubagentTimerRef.current) { clearTimeout(flushSubagentTimerRef.current); flushSubagentTimerRef.current = null; }
    };
  }, [refreshState, refreshMessages, hydratePending, flushMessages, flushSubagentMessages]);

  return {
    appState, messages, stats, busySessions, permissions, questions,
    sessionStatus, isLoadingMessages, isLoadingOlder, hasOlderMessages,
    totalMessageCount, watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId,
    mcpAgentActivity, presenceClients, liveActivityEvents,
    crossSessionPermissions, crossSessionQuestions,
    refreshState, refreshMessages, clearPermission, clearQuestion,
    clearMcpEditorOpen, clearMcpTerminalFocus, addOptimisticMessage, loadOlderMessages,
  };
}
