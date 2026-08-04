import { useEffect, useRef, useCallback, useState } from "react";
import {
  createEventsSSE,
  createSessionEventsSSE,
  parseOpenCodeEvent,
  fetchAppState,
  fetchSessionMessages,
  fetchSessionStats,
  fetchThemePair,
  fetchPending,
  type AppState,
  type SessionStats,
  type ActivityEvent,
  type ClientPresence,
} from "../../api";
import type { Message, MessagePart, PermissionRequest, QuestionRequest } from "../../types";
import { applyThemeToCss } from "../../utils/theme";
import { getPersistedAppearance, resolveThemeColors, storeThemePair } from "../../utils/appearance";

import type { SSEState, SessionStatus, WatcherStatus, SSEConnectionStatus } from "./types";
import { SESSION_IDLE } from "./types";
import { type MessageMap, mapToSortedArray, getMessageTime, mergeMessage, purgeOptimistic } from "./messageMap";
import { handleOpenCodeEvent, setupAppSSEListeners } from "./eventHandler";
import { formatPermissionDescription, deriveQuestionTitle, transformQuestionInfo } from "./transforms";

/** Number of messages to load per page. */
const MESSAGE_PAGE_SIZE = 50;

/** Maximum number of sessions to keep cached in memory. */
const MAX_SESSION_CACHE = 20;

/** Cached state for a previously-visited session. */
export interface CachedSession {
  messageMap: MessageMap;
  subagentMaps: Map<string, MessageMap>;
  stats: SessionStats | null;
  hasOlderMessages: boolean;
  totalMessageCount: number;
  liveActivityEvents: ActivityEvent[];
  /** Timestamp of last access — used for LRU eviction. */
  lastAccess: number;
}

/** Schedule a callback on idle — falls back to setTimeout(0) when requestIdleCallback is unavailable. */
const scheduleIdle = typeof requestIdleCallback === "function"
  ? (cb: () => void) => requestIdleCallback(cb)
  : (cb: () => void) => setTimeout(cb, 0);

export function useSSE(): SSEState {
  const [appState, setAppState] = useState<AppState | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [stats, setStats] = useState<SessionStats | null>(null);
  const [busySessions, setBusySessions] = useState<Set<string>>(new Set());
  const busySessionsRef = useRef<Set<string>>(new Set());
  useEffect(() => { busySessionsRef.current = busySessions; }, [busySessions]);
  const [sessionStatuses, setSessionStatuses] = useState<Record<string, SessionStatus>>({});
  const sessionStatusesRef = useRef<Record<string, SessionStatus>>({});
  useEffect(() => { sessionStatusesRef.current = sessionStatuses; }, [sessionStatuses]);
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [sessionStatus, setSessionStatus] = useState<SessionStatus>(SESSION_IDLE);
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
  const [connectionStatus, setConnectionStatus] = useState<SSEConnectionStatus>("reconnecting");
  const activeSessionRef = useRef<string | null>(null);
  /** Ids of questions the user just resolved. Guards against a racing hydratePending()
   *  or a re-delivered question.asked re-adding an already-answered question (the SSE
   *  clear and the mirror purge may lag the user's action by a beat). */
  const resolvedQuestionIdsRef = useRef<Set<string>>(new Set());
  /** Optimistic override — set instantly by beginSessionSwitch() so the sidebar
   *  and prompt react immediately, cleared once the server confirms via SSE. */
  const [activeSessionIdOverride, setActiveSessionIdOverride] = useState<string | null>(null);
  /** Optimistic project index override — set when switching to a session in a
   *  different project so the project pill updates immediately. */
  const [activeProjectIndexOverride, setActiveProjectIndexOverride] = useState<number | null>(null);
  const appliedTitleRef = useRef<string | null>(null);
  /** Target session ID of the next expected switch (or "*" to accept any).
   *  Set by user-initiated actions (selectSession, newSession, switchProject)
   *  and cleared after the switch happens.  Background SSE-driven refreshState()
   *  calls will not switch sessions unless the incoming sid matches this target
   *  or activeSession is null.
   *  Using a target ID instead of a boolean prevents stale flags from allowing
   *  unrelated session switches (e.g. a concurrent session.created SSE event). */
  const expectSessionSwitchRef = useRef<string | null>(null);
  /** Timeout handle for clearing a stale expectSessionSwitch target. */
  const expectSwitchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** Timestamp until which background SSE session adoption is blocked. */
  const blockSessionAdoptionUntilRef = useRef(0);
  /** Timeout handle for clearing a stale background-adoption block. */
  const blockSessionAdoptionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const sessionGenRef = useRef(0);
  /** Tracks the server-reported active project index — compared against urlProjectIndex
   *  to block refreshState() from drifting the active project. */
  const activeProjectIndexRef = useRef(0);

  // Keep refs in sync so reclassifyInteractions can read current values synchronously.
  const permissionsRef = useRef<PermissionRequest[]>([]);
  const questionsRef = useRef<QuestionRequest[]>([]);
  const crossPermissionsRef = useRef<PermissionRequest[]>([]);
  const crossQuestionsRef = useRef<QuestionRequest[]>([]);
  useEffect(() => { permissionsRef.current = permissions; }, [permissions]);
  useEffect(() => { questionsRef.current = questions; }, [questions]);
  useEffect(() => { crossPermissionsRef.current = crossSessionPermissions; }, [crossSessionPermissions]);
  useEffect(() => { crossQuestionsRef.current = crossSessionQuestions; }, [crossSessionQuestions]);

  /** Reclassify all pending permissions/questions when the active session changes.
   *  Items belonging to `newSid` become active; everything else becomes cross-session. */
  const reclassifyInteractions = useCallback((newSid: string | null) => {
    const pLen = permissionsRef.current.length;
    const cpLen = crossPermissionsRef.current.length;
    const qLen = questionsRef.current.length;
    const cqLen = crossQuestionsRef.current.length;
    // Nothing to reclassify — skip 4 no-op setState calls
    if (pLen === 0 && cpLen === 0 && qLen === 0 && cqLen === 0) return;
    const allPerms = pLen > 0 && cpLen > 0 ? [...permissionsRef.current, ...crossPermissionsRef.current]
      : pLen > 0 ? permissionsRef.current : crossPermissionsRef.current;
    const allQs = qLen > 0 && cqLen > 0 ? [...questionsRef.current, ...crossQuestionsRef.current]
      : qLen > 0 ? questionsRef.current : crossQuestionsRef.current;
    setPermissions(newSid ? allPerms.filter((p) => p.sessionID === newSid) : []);
    setCrossSessionPermissions(newSid ? allPerms.filter((p) => p.sessionID !== newSid) : allPerms);
    setQuestions(newSid ? allQs.filter((q) => q.sessionID === newSid) : []);
    setCrossSessionQuestions(newSid ? allQs.filter((q) => q.sessionID !== newSid) : allQs);
  }, []);
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
      // Seed sessionStatuses from busy_sessions (server only reports busy IDs — no retry detail here)
      setSessionStatuses((prev) => {
        let changed = false;
        const next = { ...prev };
        for (const p of s.projects) {
          for (const sid of p.busy_sessions) {
            if (!next[sid] || next[sid].type === "idle") {
              next[sid] = { type: "busy" };
              changed = true;
            }
          }
        }
        return changed ? next : prev;
      });
    } catch (e) {
      console.error("Failed to fetch state:", e);
    }
  }, []);

  /** Update a single session's metadata in the local app state without a full refresh.
   *  This avoids active_session drift that causes unwanted session switches. */
  const updateSessionMeta = useCallback((sessionInfo: Record<string, unknown>) => {
    const sid = (sessionInfo.id as string) || "";
    if (!sid) return;
    setAppState((prev) => {
      if (!prev) return prev;
      let changed = false;
      const projects = prev.projects.map((proj: any) => {
        const sessions = proj.sessions.map((s: any) => {
          if (s.id !== sid) return s;
          changed = true;
          return {
            ...s,
            title: (sessionInfo.title as string) ?? s.title,
            time: sessionInfo.time ? {
              created: (sessionInfo.time as any).created ?? s.time?.created,
              updated: (sessionInfo.time as any).updated ?? s.time?.updated,
            } : s.time,
          };
        });
        return changed ? { ...proj, sessions } : proj;
      });
      return changed ? { ...prev, projects } : prev;
    });
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
        // Skip questions the user already resolved — the server mirror may not have
        // purged them yet, and re-adding here is the classic "answered question reappears".
        if (resolvedQuestionIdsRef.current.has(q.id)) continue;
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
          map.set(id, mergeMessage(existing, msg));
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
    // Remember it as resolved (bounded) so a hydrate/re-ask can't bring it back, and clear
    // it from BOTH lists — a question can be classified cross-session by an early event.
    const resolved = resolvedQuestionIdsRef.current;
    resolved.add(id);
    if (resolved.size > 500) resolved.delete(resolved.values().next().value as string);
    setQuestions((prev) => prev.filter((q) => q.id !== id));
    setCrossSessionQuestions((prev) => prev.filter((q) => q.id !== id));
  }, []);
  const clearMcpEditorOpen = useCallback(() => {
    setMcpEditorOpenPath(null); setMcpEditorOpenLine(null);
  }, []);
  // Imperatively open a file in the editor panel (e.g. clicking a tool-card path).
  // Reuses the same state the MCP editor-open event drives, so the panel auto-opens.
  const openMcpEditor = useCallback((path: string, line?: number | null) => {
    if (!path) return;
    setMcpEditorOpenLine(line ?? null);
    setMcpEditorOpenPath(path);
  }, []);
  const clearMcpTerminalFocus = useCallback(() => { setMcpTerminalFocusId(null); }, []);

  const addOptimisticMessage = useCallback((text: string, images?: { base64: string; mimeType: string; name: string }[]) => {
    const id = `__optimistic__${Date.now()}`;
    const parts: MessagePart[] = [{ type: "text", text }];
    if (images) {
      for (const img of images) {
        parts.push({ type: "file", mime: img.mimeType, url: `data:${img.mimeType};base64,${img.base64}`, filename: img.name });
      }
    }
    const msg: Message = {
      info: { role: "user", messageID: id, id, sessionID: activeSessionRef.current ?? undefined, time: Date.now() / 1000 },
      parts,
    };
    messageMapRef.current.set(id, msg);
    flushMessages();
  }, [flushMessages]);

  const clearOptimistic = useCallback(() => {
    if (purgeOptimistic(messageMapRef.current)) flushMessages();
  }, [flushMessages]);

  // ── Track active session changes ──────────────────────────────
  // activeSessionRef tracks the server's reported active session. Guard against
  // unwanted switches: only allow the server to change active_session when either
  // (a) no session is active yet, (b) a session switch is expected, or (c) the
  // incoming sid matches the expected target.  Also guard against project drift —
  // if the server's active_project differs from the URL project, ignore it.
  useEffect(() => {
    if (!appState) return;
    const serverProjIdx = appState.active_project;
    const proj = appState.projects[serverProjIdx];
    const sid = proj?.active_session ?? null;
    if (sid !== activeSessionRef.current) {
      const expected = expectSessionSwitchRef.current;
      if (Date.now() < blockSessionAdoptionUntilRef.current) return;
      if (sid !== null) {
        if (activeSessionRef.current !== null) {
          if (expected === null) return;
          if (expected !== "*" && expected !== sid) return;
        }
        if (serverProjIdx !== activeProjectIndexRef.current) return;
      }
      expectSessionSwitchRef.current = null;
      if (expectSwitchTimerRef.current) {
        clearTimeout(expectSwitchTimerRef.current);
        expectSwitchTimerRef.current = null;
      }
      // Clear optimistic overrides — server has confirmed the session switch
      setActiveSessionIdOverride(null);
      setActiveProjectIndexOverride(null);

      // Save current session to cache before switching away
      saveCurrentSessionToCache();

      sessionGenRef.current += 1;
      const gen = sessionGenRef.current;
      activeSessionRef.current = sid;
      activeProjectIndexRef.current = serverProjIdx;

      // Immediately recompute sessionStatus from the authoritative busySessions set.
      // Without this, switching to an idle session keeps the previous session's
      // "busy" status (showing a stale stop button).
      setBusySessions((prev) => {
        setSessionStatus(sid && prev.has(sid)
          ? (sessionStatusesRef.current[sid] ?? { type: "busy" })
          : SESSION_IDLE);
        return prev;
      });

      // Reclassify permissions/questions based on new active session.
      // Items belonging to the new active session move to the active arrays;
      // everything else goes to cross-session.  This prevents stale questions
      // from a previous session from lingering in the inline dock.
      reclassifyInteractions(sid);

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
                  map.set(id, mergeMessage(existing, msg));
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
        // Hydrate pending permissions/questions — deferred so message rendering isn't blocked
        scheduleIdle(() => hydratePending());
      } else {
        messageMapRef.current = new Map();
        subagentMapsRef.current = new Map();
        setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0); setLiveActivityEvents([]);
        setSubagentMessages(new Map());
        setIsLoadingMessages(false);
      }
    }
  }, [appState, saveCurrentSessionToCache, restoreSessionFromCache, reclassifyInteractions]);

  // ── SSE connections (set up once on mount) ────────────────────
  useEffect(() => {
    refreshState();
    fetchThemePair().then((pair) => {
      if (!pair) return;
      const appearance = getPersistedAppearance();
      storeThemePair(pair, appearance);
      applyThemeToCss(resolveThemeColors(pair, appearance));
    });

    let lastEventTime = Date.now();
    let sessionSseNeedsRecovery = false;
    const touchEvent = () => { lastEventTime = Date.now(); };
    const recoverAfterReconnect = () => {
      console.info("[SSE] Recovering after reconnection");
      refreshState(); refreshMessages(); hydratePending();
    };

    // ── Connection status tracking ──────────────────────────────
    // Track each stream independently; aggregate to worst-case for UI.
    let appStreamOk = false;
    let sessionStreamOk = false;
    let appStreamReconnecting = false;
    let sessionStreamReconnecting = false;

    const recomputeConnectionStatus = () => {
      let next: SSEConnectionStatus;
      if (appStreamOk && sessionStreamOk) {
        next = "connected";
      } else if (appStreamReconnecting || sessionStreamReconnecting) {
        next = "reconnecting";
      } else {
        next = "disconnected";
      }
      setConnectionStatus(next);
    };

    // ── EventSource lifecycle helpers ───────────────────────────
    // Hold current EventSources in mutable slots so the watchdog can
    // close and recreate them when the connection goes stale.
    let currentAppSSE: EventSource | null = null;
    let currentSessionSSE: EventSource | null = null;

    const appSSECtx: Parameters<typeof setupAppSSEListeners>[1] = {
      activeSessionRef, sessionCacheRef, refreshState, touchEvent, recoverAfterReconnect,
      setBusySessions, setSessionStatus, setSessionStatuses, setStats, setWatcherStatus,
      setMcpEditorOpenPath, setMcpEditorOpenLine, setMcpTerminalFocusId,
      setMcpAgentActivity, setPresenceClients, setLiveActivityEvents,
    };

    function createAndWireAppSSE(): EventSource {
      const sse = createEventsSSE();
      setupAppSSEListeners(sse, appSSECtx);
      sse.addEventListener("open", () => {
        appStreamOk = true; appStreamReconnecting = false;
        recomputeConnectionStatus();
      });
      sse.addEventListener("error", () => {
        appStreamOk = false; appStreamReconnecting = true;
        recomputeConnectionStatus();
      });
      return sse;
    }

    function createAndWireSessionSSE(): EventSource {
      const sse = createSessionEventsSSE();
      sessionSseNeedsRecovery = false;
      sse.addEventListener("heartbeat", () => { touchEvent(); });
      sse.addEventListener("lagged", () => { console.warn("[SSE] Session events lagged"); recoverAfterReconnect(); });
      sse.addEventListener("error", () => {
        console.warn("[SSE] Session events connection error");
        sessionSseNeedsRecovery = true;
        sessionStreamOk = false; sessionStreamReconnecting = true;
        recomputeConnectionStatus();
      });
      sse.addEventListener("open", () => {
        touchEvent();
        sessionStreamOk = true; sessionStreamReconnecting = false;
        recomputeConnectionStatus();
        if (sessionSseNeedsRecovery) { sessionSseNeedsRecovery = false; recoverAfterReconnect(); }
      });
      sse.addEventListener("opencode", (e: MessageEvent) => {
        touchEvent();
        const event = parseOpenCodeEvent(e.data);
        if (!event) return;
        handleOpenCodeEvent(
          { activeSessionRef, messageMapRef, subagentMapsRef, sessionCacheRef,
            flushMessages, flushSubagentMessages,
            refreshState, updateSessionMeta, setStats, setSessionStatus, setBusySessions, setSessionStatuses, setPermissions, setQuestions,
            setCrossSessionPermissions, setCrossSessionQuestions, setFileEditCount, resolvedQuestionIdsRef },
          event,
        );
      });
      return sse;
    }

    currentAppSSE = createAndWireAppSSE();
    currentSessionSSE = createAndWireSessionSSE();

    // Stale-connection watchdog — closes and recreates both EventSources
    // when no events have been received for too long.
    const STALE_THRESHOLD_MS = 45_000;
    const watchdogInterval = setInterval(() => {
      const elapsed = Date.now() - lastEventTime;
      if (elapsed > STALE_THRESHOLD_MS) {
        console.warn(`[SSE] No events in ${Math.round(elapsed / 1000)}s — closing and recreating EventSources`);
        // Mark both as reconnecting
        appStreamOk = false; sessionStreamOk = false;
        appStreamReconnecting = true; sessionStreamReconnecting = true;
        recomputeConnectionStatus();
        // Close stale connections and create fresh ones
        currentAppSSE?.close();
        currentSessionSSE?.close();
        currentAppSSE = createAndWireAppSSE();
        currentSessionSSE = createAndWireSessionSSE();
        lastEventTime = Date.now();
        recoverAfterReconnect();
      }
    }, 10_000);

    return () => {
      currentAppSSE?.close(); currentSessionSSE?.close(); clearInterval(watchdogInterval);
      if (flushTimerRef.current) { clearTimeout(flushTimerRef.current); flushTimerRef.current = null; }
      if (flushSubagentTimerRef.current) { clearTimeout(flushSubagentTimerRef.current); flushSubagentTimerRef.current = null; }
      if (expectSwitchTimerRef.current) { clearTimeout(expectSwitchTimerRef.current); expectSwitchTimerRef.current = null; }
      if (blockSessionAdoptionTimerRef.current) { clearTimeout(blockSessionAdoptionTimerRef.current); blockSessionAdoptionTimerRef.current = null; }
    };
  }, [refreshState, updateSessionMeta, refreshMessages, hydratePending, flushMessages, flushSubagentMessages]);

  /** Signal that a user-initiated session switch is expected (call before selectSession/newSession).
   *  Clears any optimistic override so the UI falls back to server state. */
  const expectSessionSwitch = useCallback(() => {
    expectSessionSwitchRef.current = "*";
    // Safety net: clear stale flag after 10s so a failed API call can't leave
    // the guard permanently open.
    if (expectSwitchTimerRef.current) clearTimeout(expectSwitchTimerRef.current);
    expectSwitchTimerRef.current = setTimeout(() => {
      expectSessionSwitchRef.current = null;
      expectSwitchTimerRef.current = null;
    }, 10_000);
    setActiveSessionIdOverride(null);
    setActiveProjectIndexOverride(null);
  }, []);

  const blockSessionAdoption = useCallback((ms = 10_000) => {
    blockSessionAdoptionUntilRef.current = Date.now() + ms;
    if (blockSessionAdoptionTimerRef.current) clearTimeout(blockSessionAdoptionTimerRef.current);
    blockSessionAdoptionTimerRef.current = setTimeout(() => {
      blockSessionAdoptionUntilRef.current = 0;
      blockSessionAdoptionTimerRef.current = null;
    }, ms);
  }, []);

  /** Stable callback for checking busy state — avoids passing Set reference to children. */
  const isSessionBusy = useCallback((sid: string) => busySessionsRef.current.has(sid), []);

  /** Optimistically begin a session switch — immediately clears messages and shows loading state.
   *  Call this at click-time before any async API calls so the UI responds instantly.
   *  Also kicks off the message fetch so data loads in parallel with the API round-trips.
   *  Pass `projectIdx` when switching to a session in a different project. */
  const beginSessionSwitch = useCallback((targetSid: string, projectIdx?: number) => {
    // Set the specific target — the guard will only allow this exact session
    expectSessionSwitchRef.current = targetSid;
    if (expectSwitchTimerRef.current) clearTimeout(expectSwitchTimerRef.current);
    expectSwitchTimerRef.current = setTimeout(() => {
      expectSessionSwitchRef.current = null;
      expectSwitchTimerRef.current = null;
    }, 10_000);
    saveCurrentSessionToCache();
    sessionGenRef.current += 1;
    const gen = sessionGenRef.current;
    activeSessionRef.current = targetSid;
    // Update the project index ref immediately so the appState guard at line 463
    // (serverProjIdx !== activeProjectIndexRef.current) accepts the server's
    // confirmation of a cross-project switch.  Without this, the guard rejects
    // the valid state_changed event because the ref still holds the old project.
    if (projectIdx !== undefined) {
      activeProjectIndexRef.current = projectIdx;
    }
    // Optimistic override — makes sidebar highlight and prompt react instantly
    setActiveSessionIdOverride(targetSid);
    setActiveProjectIndexOverride(projectIdx ?? null);

    // Recompute session status immediately
    setBusySessions((prev) => {
      setSessionStatus(prev.has(targetSid)
        ? (sessionStatusesRef.current[targetSid] ?? { type: "busy" })
        : SESSION_IDLE);
      return prev;
    });
    reclassifyInteractions(targetSid);

    // Try cache first — if hit, show cached data instantly (no shimmer)
    const restored = restoreSessionFromCache(targetSid);
    if (restored) {
      setIsLoadingMessages(false);
      // Background-refresh to pick up messages that arrived while inactive
      fetchSessionMessages(targetSid, { limit: MESSAGE_PAGE_SIZE })
        .then((resp) => {
          if (gen !== sessionGenRef.current) return;
          const map = messageMapRef.current;
          let changed = false;
          for (const msg of resp.messages) {
            const id = msg.info.messageID || msg.info.id || "";
            if (!id) continue;
            const existing = map.get(id);
            if (!existing) { map.set(id, msg); changed = true; }
            else {
              map.set(id, mergeMessage(existing, msg));
              changed = true;
            }
          }
          if (changed) setMessages(mapToSortedArray(map));
          setHasOlderMessages(resp.has_more); setTotalMessageCount(resp.total);
        })
        .catch(() => {});
    } else {
      // Cache miss — clear and show loading shimmer
      messageMapRef.current = new Map();
      subagentMapsRef.current = new Map();
      setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0);
      setLiveActivityEvents([]); setSubagentMessages(new Map());
      setIsLoadingMessages(true);
      fetchSessionMessages(targetSid, { limit: MESSAGE_PAGE_SIZE })
        .then((resp) => {
          if (gen !== sessionGenRef.current) return;
          const newMap: MessageMap = new Map();
          for (const msg of resp.messages) { const id = msg.info.messageID || msg.info.id || ""; if (id) newMap.set(id, msg); }
          messageMapRef.current = newMap;
          setMessages(mapToSortedArray(newMap)); setHasOlderMessages(resp.has_more); setTotalMessageCount(resp.total);
        })
        .catch(() => { if (gen !== sessionGenRef.current) return; setMessages([]); })
        .finally(() => { if (gen !== sessionGenRef.current) return; setIsLoadingMessages(false); });
    }
    fetchSessionStats(targetSid).then((st) => { if (gen !== sessionGenRef.current) return; setStats(st); }).catch(() => {});
    // Deferred — permissions/questions are not on the critical path for message rendering
    scheduleIdle(() => hydratePending());
  }, [saveCurrentSessionToCache, restoreSessionFromCache, reclassifyInteractions, hydratePending]);

  return {
    appState, messages, stats, busySessions, sessionStatuses, permissions, questions,
    sessionStatus, connectionStatus,
    isLoadingMessages, isLoadingOlder, hasOlderMessages,
    totalMessageCount, watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId,
    mcpAgentActivity, presenceClients, liveActivityEvents,
    crossSessionPermissions, crossSessionQuestions,
    refreshState, refreshMessages, clearPermission, clearQuestion,
    clearMcpEditorOpen, openMcpEditor, clearMcpTerminalFocus, addOptimisticMessage, clearOptimistic, loadOlderMessages,
    expectSessionSwitch, blockSessionAdoption, beginSessionSwitch, isSessionBusy,
  };
}
