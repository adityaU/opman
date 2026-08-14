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
  type ClientPresence,
} from "../../api";
import type { Message, MessagePart, PermissionRequest, QuestionRequest } from "../../types";
import { applyThemeToCss } from "../../utils/theme";
import { getPersistedAppearance, resolveThemeColors, storeThemePair } from "../../utils/appearance";

import type { SSEState, SessionStatus, WatcherStatus, SSEConnectionStatus } from "./types";
import { SESSION_IDLE } from "./types";
import { type MessageMap, mapToSortedArray, getMessageTime, mergeMessage } from "./messageMap";
import {
  createOptimisticId, purgeOptimistic, retainOptimistic, reconcileOptimistic,
  stashOptimistic, takeOptimistic, dropStashedOptimistic, purgeForeignOptimistic,
  type OptimisticStash,
} from "./optimistic";
import { handleOpenCodeEvent, setupAppSSEListeners } from "./eventHandler";
import {
  dropSession,
  isSessionPinned,
  publishSession,
  setSessionOlderLoader,
  setSessionDemandHandler,
} from "./sessionStore";
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
  const [mcpBrowserOpen, setMcpBrowserOpen] = useState<
    { projectPath: string; url: string } | null
  >(null);
  const [mcpAgentActivity, setMcpAgentActivity] = useState<Map<string, boolean>>(new Map());
  const [presenceClients, setPresenceClients] = useState<ClientPresence[]>([]);
  const [crossSessionPermissions, setCrossSessionPermissions] = useState<PermissionRequest[]>([]);
  const [crossSessionQuestions, setCrossSessionQuestions] = useState<QuestionRequest[]>([]);
  const [connectionStatus, setConnectionStatus] = useState<SSEConnectionStatus>("reconnecting");
  const [initialConnectionsReady, setInitialConnectionsReady] = useState(false);
  const startupReadyRef = useRef(false);
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
  // Placeholders for sends aimed at a session that is not on screen. Parked
  // here rather than in the live map, which belongs to whatever session is
  // displayed — and is the very object that session's cache entry holds.
  const optimisticStashRef = useRef<OptimisticStash>(new Map());

  /** Mirror of React state values needed by the cache save function (refs can be read synchronously). */
  const statsRef = useRef<SessionStats | null>(null);
  const hasOlderRef = useRef(false);
  const totalCountRef = useRef(0);

  // Keep cache-related refs in sync with React state
  useEffect(() => { statsRef.current = stats; }, [stats]);
  useEffect(() => { hasOlderRef.current = hasOlderMessages; }, [hasOlderMessages]);
  useEffect(() => { totalCountRef.current = totalMessageCount; }, [totalMessageCount]);

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
      lastAccess: Date.now(),
    });
    // LRU eviction. A session a pane is watching is exempt: evicting it would
    // blank a visible transcript and, worse, silently stop applying its
    // events — the pane would look idle while its agent was still working.
    if (cache.size > MAX_SESSION_CACHE) {
      let oldestKey: string | null = null;
      let oldestTime = Infinity;
      for (const [key, entry] of cache) {
        if (isSessionPinned(key)) continue;
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
    // A cached map is adopted wholesale, so it is the one place a stray
    // placeholder from another session could come back to life.
    purgeForeignOptimistic(cached.messageMap, sid);
    messageMapRef.current = cached.messageMap;
    subagentMapsRef.current = cached.subagentMaps;
    setMessages(mapToSortedArray(cached.messageMap));
    setStats(cached.stats);
    setHasOlderMessages(cached.hasOlderMessages);
    setTotalMessageCount(cached.totalMessageCount);
    // Flush subagent messages
    const result = new Map<string, Message[]>();
    for (const [subSid, map] of cached.subagentMaps) {
      result.set(subSid, mapToSortedArray(map));
    }
    setSubagentMessages(result);
    return true;
  }, []);

  // ── Per-session publishing ────────────────────────────────────
  /**
   * Push a session's current state to whoever is watching it.
   *
   * Reads the active session from the live refs and any other from its cache
   * entry, so a pane showing a background session sees exactly what the event
   * handler has already written there. Unwatched sessions cost nothing: the
   * store drops them before any array is built.
   */
  const publishOne = useCallback((sid: string) => {
    if (!isSessionPinned(sid)) return;
    const active = sid === activeSessionRef.current;
    const cached = active ? null : sessionCacheRef.current.get(sid);
    if (!active && !cached) return; // not loaded yet — hydration will publish

    const map = active ? messageMapRef.current : cached!.messageMap;
    publishSession(sid, {
      messages: mapToSortedArray(map),
      stats: active ? statsRef.current : cached!.stats,
      status: sessionStatusesRef.current[sid] ?? SESSION_IDLE,
      loading: false,
      hasOlder: active ? hasOlderRef.current : cached!.hasOlderMessages,
      total: active ? totalCountRef.current : cached!.totalMessageCount,
    });
  }, []);

  /**
   * Coalesce publishes across sessions into one frame.
   *
   * Several sessions can stream at once, and each of their events would
   * otherwise schedule its own timer; one timer with a dirty set means N busy
   * agents cost one flush per frame rather than N.
   */
  const publishTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dirtySessionsRef = useRef<Set<string>>(new Set());
  const notifySession = useCallback((sid: string) => {
    if (!sid) return;
    dirtySessionsRef.current.add(sid);
    if (publishTimerRef.current) return;
    publishTimerRef.current = setTimeout(() => {
      publishTimerRef.current = null;
      const dirty = dirtySessionsRef.current;
      dirtySessionsRef.current = new Set();
      for (const id of dirty) publishOne(id);
    }, 16);
  }, [publishOne]);

  /**
   * Load a session someone just started watching but nobody has opened.
   *
   * Its transcript goes into the same LRU cache the event handler already
   * writes background sessions into, so from here on it stays current without
   * any further fetching.
   */
  const hydrateWatchedSession = useCallback(async (sid: string) => {
    if (sid === activeSessionRef.current || sessionCacheRef.current.has(sid)) {
      publishOne(sid);
      return;
    }
    try {
      const resp = await fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE });
      // Another watcher may have hydrated it while this request was in flight.
      if (!sessionCacheRef.current.has(sid)) {
        const map: MessageMap = new Map();
        for (const msg of resp.messages) {
          const id = msg.info.messageID || msg.info.id || "";
          if (id) map.set(id, msg);
        }
        sessionCacheRef.current.set(sid, {
          messageMap: map,
          subagentMaps: new Map(),
          stats: null,
          hasOlderMessages: resp.has_more,
          totalMessageCount: resp.total,
          lastAccess: Date.now(),
        });
      }
    } catch (e) {
      console.error("hydrateWatchedSession failed:", e);
      // Publish anyway so the pane stops saying "loading" and can show its
      // own empty state rather than a spinner that never resolves.
      publishSession(sid, {
        messages: [],
        stats: null,
        status: SESSION_IDLE,
        loading: false,
        hasOlder: false,
        total: 0,
      });
      return;
    }
    publishOne(sid);
  }, [publishOne]);

  useEffect(() => {
    setSessionDemandHandler((sid) => { void hydrateWatchedSession(sid); });
    return () => setSessionDemandHandler(null);
  }, [hydrateWatchedSession]);

  // ── Flush helpers (debounced to ~1 frame) ─────────────────────
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushMessages = useCallback(() => {
    if (flushTimerRef.current) return;
    flushTimerRef.current = setTimeout(() => {
      flushTimerRef.current = null;
      setMessages(mapToSortedArray(messageMapRef.current));
      // A pane may be showing the active session too — it subscribes like any
      // other, and does not read the hook's own `messages`.
      const active = activeSessionRef.current;
      if (active) publishOne(active);
    }, 16);
  }, [publishOne]);

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
      startupReadyRef.current = s.startup_ready !== false;
      setAppState(s);
      // Set page title from instance name (tunnel subdomain) if provided — only when changed
      if (s.instance_name && s.instance_name !== appliedTitleRef.current) {
        document.title = s.instance_name;
        appliedTitleRef.current = s.instance_name;
      }
      // App state carries a full snapshot of who is busy, so reconcile against
      // it rather than merging: a merge could only ever add, leaving a session
      // marked busy for the rest of the page's life once its runner finished.
      //
      // The snapshot reports busy per project session, so it can only speak for
      // sessions it lists. One it has never heard of — a handoff or a brand-new
      // session whose first turn is already streaming — is unknown, not idle,
      // and clearing it here would drop the spinner mid-turn.
      const busyNow = new Set<string>();
      const known = new Set<string>();
      for (const p of s.projects) {
        for (const session of p.sessions) known.add(session.id);
        for (const sid of p.busy_sessions) busyNow.add(sid);
      }
      const retired = (sid: string) => known.has(sid) && !busyNow.has(sid);
      setBusySessions((prev) => {
        const next = new Set(busyNow);
        for (const sid of prev) if (!retired(sid)) next.add(sid);
        if (prev.size === next.size && [...next].every((sid) => prev.has(sid))) return prev;
        return next;
      });
      // Seed sessionStatuses from busy_sessions (server only reports busy IDs — no retry detail here)
      setSessionStatuses((prev) => {
        let changed = false;
        const next = { ...prev };
        for (const sid of busyNow) {
          if (!next[sid] || next[sid].type === "idle") {
            next[sid] = { type: "busy" };
            changed = true;
          }
        }
        for (const sid of Object.keys(next)) {
          if (!retired(sid)) continue;
          delete next[sid];
          changed = true;
        }
        return changed ? next : prev;
      });
      // The claude adapters report progress only through app state — they emit
      // no session_busy/session_idle events — so the active session's status has
      // to be recomputed here as well. Without this the composer keeps offering
      // Send and the transcript claims idle for the whole turn.
      const activeSid = activeSessionRef.current;
      if (activeSid) {
        const isBusy = busyNow.has(activeSid);
        setSessionStatus((prev) => {
          if (!isBusy) return retired(activeSid) ? SESSION_IDLE : prev;
          // Keep any richer busy detail an SSE event already supplied.
          return prev.type === "idle" ? { type: "busy" } : prev;
        });
      }
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

  /**
   * Load a session's transcript into the message map.
   *
   * Both routes into a session — the URL-driven `beginSessionSwitch` and the
   * server-confirmed appState effect — go through here so they cannot drift
   * apart. Optimistic placeholders for the target session survive the load: a
   * session created by its own first send has no transcript yet, so the
   * placeholder is the only record of the prompt. The claude runners never
   * write user messages at all, which makes it the only record there is.
   */
  const hydrateSession = useCallback((sid: string, gen: number) => {
    // Read placeholders before restoring, which replaces the map wholesale.
    const pending = retainOptimistic(messageMapRef.current, sid);
    for (const [key, msg] of takeOptimistic(optimisticStashRef.current, sid)) {
      pending.set(key, msg);
    }
    const cached = restoreSessionFromCache(sid);

    if (cached) {
      for (const [key, msg] of pending) messageMapRef.current.set(key, msg);
      if (pending.size > 0) setMessages(mapToSortedArray(messageMapRef.current));
    } else {
      messageMapRef.current = pending;
      subagentMapsRef.current = new Map();
      setMessages(mapToSortedArray(pending));
      setHasOlderMessages(false); setTotalMessageCount(0);
      setSubagentMessages(new Map());
    }
    // A placeholder already fills the view, so a shimmer would only flash.
    setIsLoadingMessages(!cached && pending.size === 0);

    fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE })
      .then((resp) => {
        if (gen !== sessionGenRef.current) return;
        const map = messageMapRef.current;
        for (const msg of resp.messages) {
          const id = msg.info.messageID || msg.info.id || "";
          if (!id) continue;
          const existing = map.get(id);
          map.set(id, existing ? mergeMessage(existing, msg) : msg);
        }
        reconcileOptimistic(map);
        setMessages(mapToSortedArray(map));
        setHasOlderMessages(resp.has_more);
        setTotalMessageCount(resp.total);
      })
      .catch(() => {
        if (gen !== sessionGenRef.current) return;
        setMessages(mapToSortedArray(messageMapRef.current));
      })
      .finally(() => { if (gen !== sessionGenRef.current) return; setIsLoadingMessages(false); });

    fetchSessionStats(sid).then((st) => { if (gen !== sessionGenRef.current) return; setStats(st); }).catch(() => {});
  }, [restoreSessionFromCache]);

  /**
   * Re-read a session's transcript from the server.
   *
   * `adoptView` decides what happens when the requested session is not the one
   * on screen. A session the caller just created must be adopted — the view is
   * already being moved there. A send on an *existing* session must not be: the
   * user may have opened another conversation while the request was in flight,
   * and taking the view back would yank them out of it. In that case the fetch
   * goes into the target's cached transcript, silently, or is skipped when the
   * session has no cache entry to update (opening it fetches anyway).
   */
  const refreshMessages = useCallback(async (
    requestedSessionId?: string | null,
    options?: { adoptView?: boolean },
  ) => {
    const adoptView = options?.adoptView ?? true;
    if (requestedSessionId && requestedSessionId !== activeSessionRef.current && !adoptView) {
      const cached = sessionCacheRef.current.get(requestedSessionId);
      if (!cached) return;
      try {
        const resp = await fetchSessionMessages(requestedSessionId, { limit: MESSAGE_PAGE_SIZE });
        const map = cached.messageMap;
        for (const msg of resp.messages) {
          const id = msg.info.messageID || msg.info.id || "";
          if (!id) continue;
          const existing = map.get(id);
          map.set(id, existing ? mergeMessage(existing, msg) : msg);
        }
        reconcileOptimistic(map);
        cached.totalMessageCount = resp.total;
        cached.hasOlderMessages = resp.has_more;
      } catch (e) {
        console.error("refreshMessages (background) failed:", e);
      }
      return;
    }
    if (requestedSessionId && requestedSessionId !== activeSessionRef.current) {
      // A lazy new-session send can finish while the previous session's fetch
      // is still in flight. Move the ref and generation forward first so that
      // stale results cannot replace the new session's optimistic first turn.
      //
      // The map has to move with the ref. It still holds the session being left
      // behind — and is the very object that session's cache entry was stored
      // by reference — so writing this session's transcript into it merges the
      // two conversations in both directions: the new session renders the old
      // one's turns, and the old one keeps the new one's for the rest of the
      // page's life. Every SSE handler decides *whether* to write from the ref
      // and *where* to write from the map, so a window where they name
      // different sessions also routes live events into the wrong transcript.
      saveCurrentSessionToCache();
      sessionGenRef.current += 1;
      activeSessionRef.current = requestedSessionId;
      const pending = retainOptimistic(messageMapRef.current, requestedSessionId);
      for (const [key, msg] of takeOptimistic(optimisticStashRef.current, requestedSessionId)) {
        pending.set(key, msg);
      }
      if (restoreSessionFromCache(requestedSessionId)) {
        for (const [key, msg] of pending) messageMapRef.current.set(key, msg);
      } else {
        messageMapRef.current = pending;
        subagentMapsRef.current = new Map();
        setSubagentMessages(new Map());
        setHasOlderMessages(false);
        setTotalMessageCount(0);
      }
      setMessages(mapToSortedArray(messageMapRef.current));
    }
    const sid = requestedSessionId ?? activeSessionRef.current;
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
      // The transcript now owns any prompt it has written — retire the local
      // placeholder for it so the turn is not shown twice.
      reconcileOptimistic(map);
      setMessages(mapToSortedArray(map));
      setHasOlderMessages(resp.has_more);
      setTotalMessageCount(resp.total);
    } catch (e) {
      console.error("refreshMessages failed:", e);
    }
  }, [saveCurrentSessionToCache, restoreSessionFromCache]);

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

  /**
   * Page any watched session backwards, active or not.
   *
   * A pane showing a background session has the same long transcript as the
   * foreground one, so pagination cannot be a property of being active. The
   * active session still routes through `loadOlderMessages` — it owns the
   * hook's own `messages` state, and two writers of that array would race.
   */
  const loadOlderIn = useCallback(async (sid: string): Promise<boolean> => {
    if (!sid) return false;
    if (sid === activeSessionRef.current) return loadOlderMessages();

    const cached = sessionCacheRef.current.get(sid);
    if (!cached || !cached.hasOlderMessages) return false;
    let oldestTs = Infinity;
    for (const msg of cached.messageMap.values()) {
      const ts = getMessageTime(msg);
      if (ts > 0 && ts < oldestTs) oldestTs = ts;
    }
    if (oldestTs === Infinity) return false;
    try {
      const resp = await fetchSessionMessages(sid, { limit: MESSAGE_PAGE_SIZE, before: oldestTs });
      // The session may have been evicted or become active while in flight.
      const live = sessionCacheRef.current.get(sid);
      if (!live || live !== cached) return false;
      for (const msg of resp.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id && !live.messageMap.has(id)) live.messageMap.set(id, msg);
      }
      live.hasOlderMessages = resp.has_more;
      publishOne(sid);
      return resp.has_more;
    } catch {
      return false;
    }
  }, [loadOlderMessages, publishOne]);

  useEffect(() => {
    setSessionOlderLoader(loadOlderIn);
    return () => setSessionOlderLoader(null);
  }, [loadOlderIn]);

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
  const clearMcpBrowserOpen = useCallback(() => { setMcpBrowserOpen(null); }, []);

  /**
   * Show a submitted prompt immediately, in the session it was sent to.
   *
   * The target is decided by `sessionId`, never by which transcript happens to
   * be on screen: a lazily created session is not hydrated until an effect runs,
   * and the user may navigate away while the request is in flight. Writing into
   * the live map in either case put the message in another conversation for good
   * — the map is shared by reference with that session's cache entry, so
   * reopening it brought the foreign turn back every time.
   *
   * Returns the placeholder's id so the caller can retire exactly the one it
   * created if the send fails.
   */
  const addOptimisticMessage = useCallback((text: string, images?: { base64: string; mimeType: string; name: string }[], sessionId?: string | null): string | null => {
    const target = sessionId ?? activeSessionRef.current;
    // An unstamped placeholder belongs to no session and could only ever be
    // removed by a blanket purge. There is nothing useful to show it against.
    if (!target) return null;

    const id = createOptimisticId();
    const parts: MessagePart[] = [{ type: "text", text }];
    if (images) {
      for (const img of images) {
        parts.push({ type: "file", mime: img.mimeType, url: `data:${img.mimeType};base64,${img.base64}`, filename: img.name });
      }
    }
    const msg: Message = {
      // Milliseconds, matching server records — seconds here would sort the
      // placeholder ahead of the entire transcript instead of at the end.
      info: { role: "user", messageID: id, id, sessionID: target, time: Date.now() },
      parts,
    };

    if (target !== activeSessionRef.current) {
      const cached = sessionCacheRef.current.get(target);
      if (cached) cached.messageMap.set(id, msg);
      else stashOptimistic(optimisticStashRef.current, target, id, msg);
      return id;
    }

    messageMapRef.current.set(id, msg);
    flushMessages();
    return id;
  }, [flushMessages]);

  /**
   * Retire placeholders. Scoped to one placeholder when the caller names it, so
   * a failed send cannot wipe another session's — or another turn's — pending
   * prompt.
   */
  const clearOptimistic = useCallback((sessionId?: string | null, id?: string) => {
    if (!id) {
      if (purgeOptimistic(messageMapRef.current)) flushMessages();
      return;
    }
    const target = sessionId ?? activeSessionRef.current;
    if (target && target !== activeSessionRef.current) {
      dropStashedOptimistic(optimisticStashRef.current, target, id);
      sessionCacheRef.current.get(target)?.messageMap.delete(id);
      return;
    }
    if (messageMapRef.current.delete(id)) flushMessages();
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
        hydrateSession(sid, gen);
        // Hydrate pending permissions/questions — deferred so message rendering isn't blocked
        scheduleIdle(() => hydratePending());
      } else {
        // Landed on the new-session screen — nothing to show yet.
        messageMapRef.current = new Map();
        subagentMapsRef.current = new Map();
        setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0);
        setSubagentMessages(new Map());
        setIsLoadingMessages(false);
      }
    }
  }, [appState, saveCurrentSessionToCache, hydrateSession, reclassifyInteractions]);

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
    let appStreamOpened = false;
    let sessionStreamOpened = false;
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
      setMcpEditorOpenPath, setMcpEditorOpenLine, setMcpTerminalFocusId, setMcpBrowserOpen,
      setMcpAgentActivity, setPresenceClients,
    };

    function createAndWireAppSSE(): EventSource {
      const sse = createEventsSSE();
      setupAppSSEListeners(sse, appSSECtx);
      sse.addEventListener("open", () => {
        appStreamOk = true; appStreamOpened = true; appStreamReconnecting = false;
        if (sessionStreamOpened) setInitialConnectionsReady(true);
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
        sessionStreamOk = true; sessionStreamOpened = true; sessionStreamReconnecting = false;
        if (appStreamOpened) setInitialConnectionsReady(true);
        recomputeConnectionStatus();
        if (sessionSseNeedsRecovery) { sessionSseNeedsRecovery = false; recoverAfterReconnect(); }
      });
      sse.addEventListener("opencode", (e: MessageEvent) => {
        touchEvent();
        const event = parseOpenCodeEvent(e.data);
        if (!event) return;
        handleOpenCodeEvent(
          { activeSessionRef, messageMapRef, subagentMapsRef, sessionCacheRef,
            flushMessages, flushSubagentMessages, notifySession, dropSession,
            refreshState, updateSessionMeta, setStats, setSessionStatus, setBusySessions, setSessionStatuses, setPermissions, setQuestions,
            setCrossSessionPermissions, setCrossSessionQuestions, setFileEditCount, resolvedQuestionIdsRef },
          event,
        );
      });
      return sse;
    }

    currentAppSSE = createAndWireAppSSE();
    currentSessionSSE = createAndWireSessionSSE();

    // The backend may be accepting authenticated requests before its first
    // session hydration finishes. Poll the explicit readiness flag so an
    // empty snapshot never becomes an accidental new-session flow.
    const startupPoll = setInterval(() => {
      if (startupReadyRef.current) return;
      refreshState();
    }, 1000);

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
      currentAppSSE?.close(); currentSessionSSE?.close(); clearInterval(watchdogInterval); clearInterval(startupPoll);
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

    hydrateSession(targetSid, gen);
    // Deferred — permissions/questions are not on the critical path for message rendering
    scheduleIdle(() => hydratePending());
  }, [saveCurrentSessionToCache, hydrateSession, reclassifyInteractions, hydratePending]);

  return {
    appState, messages, stats, busySessions, sessionStatuses, permissions, questions,
    sessionStatus, connectionStatus, initialConnectionsReady,
    isLoadingMessages, isLoadingOlder, hasOlderMessages,
    totalMessageCount, watcherStatus, subagentMessages, fileEditCount,
    mcpEditorOpenPath, mcpEditorOpenLine, mcpTerminalFocusId, mcpBrowserOpen,
    mcpAgentActivity, presenceClients,
    crossSessionPermissions, crossSessionQuestions,
    refreshState, refreshMessages, clearPermission, clearQuestion,
    clearMcpEditorOpen, openMcpEditor, clearMcpTerminalFocus, clearMcpBrowserOpen, addOptimisticMessage, clearOptimistic, loadOlderMessages,
    expectSessionSwitch, blockSessionAdoption, beginSessionSwitch, isSessionBusy,
  };
}
