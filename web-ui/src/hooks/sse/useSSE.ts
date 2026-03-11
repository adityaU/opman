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
  type ActivityEvent,
  type ClientPresence,
} from "../../api";
import type { Message, PermissionRequest, QuestionRequest } from "../../types";
import { applyThemeToCss } from "../../utils/theme";

import type { SSEState, WatcherStatus } from "./types";
import { type MessageMap, mapToSortedArray, getMessageTime } from "./messageMap";
import { handleOpenCodeEvent, setupAppSSEListeners } from "./eventHandler";

/** Number of messages to load per page. */
const MESSAGE_PAGE_SIZE = 50;

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
      sessionGenRef.current += 1;
      const gen = sessionGenRef.current;
      activeSessionRef.current = sid;
      messageMapRef.current = new Map();
      setMessages([]); setHasOlderMessages(false); setTotalMessageCount(0); setLiveActivityEvents([]);
      if (sid) {
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
      } else { setIsLoadingMessages(false); }
    }
  }, [appState]);

  // ── SSE connections (set up once on mount) ────────────────────
  useEffect(() => {
    refreshState();
    fetchTheme().then((colors) => { if (colors) applyThemeToCss(colors); });

    let lastEventTime = Date.now();
    let sessionSseNeedsRecovery = false;
    const touchEvent = () => { lastEventTime = Date.now(); };
    const recoverAfterReconnect = () => {
      console.info("[SSE] Recovering after reconnection");
      refreshState(); refreshMessages();
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
        { activeSessionRef, messageMapRef, subagentMapsRef, flushMessages, flushSubagentMessages,
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
  }, [refreshState, refreshMessages, flushMessages, flushSubagentMessages]);

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
