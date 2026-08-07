import type { PermissionRequest, QuestionRequest, OpenCodeEvent } from "../../types";
import type { SessionStats, ClientPresence, ThemePair } from "../../api";
import { applyThemeToCss } from "../../utils/theme";
import { getPersistedAppearance, resolveThemeColors, storeThemePair } from "../../utils/appearance";
import type { WatcherStatus, McpAgentActivity, McpEditorOpen, SessionStatus } from "./types";
import { SESSION_IDLE } from "./types";
import type { CachedSession } from "./useSSE";
import { formatPermissionDescription, deriveQuestionTitle, transformQuestionInfo } from "./transforms";
import { type MessageMap, upsertMessageInfo, upsertPart, applyPartDelta, removeMessage, removePart } from "./messageMap";
import { reconcileOptimistic } from "./optimistic";

/** Setters and refs needed by the event handler — avoids passing 20+ individual args. */
export interface EventHandlerContext {
  activeSessionRef: { current: string | null };
  messageMapRef: { current: MessageMap };
  subagentMapsRef: { current: Map<string, MessageMap> };
  /** LRU cache of previously-visited sessions — events for cached sessions update in background. */
  sessionCacheRef: { current: Map<string, CachedSession> };
  flushMessages: () => void;
  flushSubagentMessages: () => void;
  refreshState: () => void;
  /** Update a single session's metadata in app state without a full refresh. */
  updateSessionMeta: (sessionInfo: Record<string, unknown>) => void;
  setStats: React.Dispatch<React.SetStateAction<SessionStats | null>>;
  setSessionStatus: React.Dispatch<React.SetStateAction<SessionStatus>>;
  setBusySessions: React.Dispatch<React.SetStateAction<Set<string>>>;
  setSessionStatuses: React.Dispatch<React.SetStateAction<Record<string, SessionStatus>>>;
  setPermissions: React.Dispatch<React.SetStateAction<PermissionRequest[]>>;
  setQuestions: React.Dispatch<React.SetStateAction<QuestionRequest[]>>;
  setCrossSessionPermissions: React.Dispatch<React.SetStateAction<PermissionRequest[]>>;
  setCrossSessionQuestions: React.Dispatch<React.SetStateAction<QuestionRequest[]>>;
  setFileEditCount: React.Dispatch<React.SetStateAction<number>>;
  /** Ids of questions the user already resolved — suppresses re-adds from a re-delivered
   *  question.asked so answered questions can't reappear. */
  resolvedQuestionIdsRef: { current: Set<string> };
}

/** Get or create a subagent message map. */
function getOrCreateSubMap(
  subagentMapsRef: { current: Map<string, MessageMap> },
  sessionId: string,
): MessageMap {
  let subMap = subagentMapsRef.current.get(sessionId);
  if (!subMap) {
    subMap = new Map();
    subagentMapsRef.current.set(sessionId, subMap);
  }
  return subMap;
}

/** Try to get the message map for a non-active session from the session cache.
 *  Returns the cached session's message map if found, otherwise null. */
function getCachedMessageMap(
  ctx: EventHandlerContext,
  sessionId: string,
): MessageMap | null {
  const cached = ctx.sessionCacheRef.current.get(sessionId);
  return cached ? cached.messageMap : null;
}

/** Get the full CachedSession entry for a non-active session, or null. */
function getCachedSession(
  ctx: EventHandlerContext,
  sessionId: string,
): CachedSession | null {
  return ctx.sessionCacheRef.current.get(sessionId) ?? null;
}

/**
 * True when an event belongs to a session other than the one the main
 * transcript is showing.
 *
 * Having no active session still counts as "not this session". On the
 * new-session screen — and during the window where a first send has created a
 * session but the server has not yet reported it active — any other session's
 * events would otherwise be treated as belonging here, splicing foreign
 * messages into the transcript and clearing the placeholder standing in for the
 * prompt just sent.
 */
function isForeignSession(ctx: EventHandlerContext, sessionId: string): boolean {
  if (!sessionId) return false;
  return sessionId !== ctx.activeSessionRef.current;
}

/** Whether a part belongs to a user message already in the transcript. */
function isUserMessagePart(ctx: EventHandlerContext, part: Record<string, unknown>): boolean {
  const messageId = (part.messageID as string) || "";
  if (!messageId) return false;
  return ctx.messageMapRef.current.get(messageId)?.info.role === "user";
}

/** Route an opencode SSE event to the appropriate React state updaters. */
export function handleOpenCodeEvent(ctx: EventHandlerContext, event: OpenCodeEvent): void {
  const props = event.properties || {};

  switch (event.type) {
    case "message.updated": {
      const info = props.info as Record<string, unknown> | undefined;
      if (!info) break;
      const msgSessionId = (info.sessionID as string) || "";

      // Route to subagent map if not the active session
      if (isForeignSession(ctx, msgSessionId)) {
        // Also update the session cache if this session is cached (background update)
        const cached = getCachedSession(ctx, msgSessionId);
        if (cached) {
          upsertMessageInfo(cached.messageMap, info);
          // Update cached stats from message cost/tokens
          if (info.cost !== undefined || info.tokens !== undefined) {
            const tokens = info.tokens as Record<string, unknown> | undefined;
            const cacheTokens = tokens?.cache as Record<string, number> | undefined;
            cached.stats = {
              cost: (info.cost as number) ?? cached.stats?.cost ?? 0,
              input_tokens: (tokens?.input as number) ?? cached.stats?.input_tokens ?? 0,
              output_tokens: (tokens?.output as number) ?? cached.stats?.output_tokens ?? 0,
              reasoning_tokens: (tokens?.reasoning as number) ?? cached.stats?.reasoning_tokens ?? 0,
              cache_read: cacheTokens?.read ?? cached.stats?.cache_read ?? 0,
              cache_write: cacheTokens?.write ?? cached.stats?.cache_write ?? 0,
            };
          }
        }

        const subMap = getOrCreateSubMap(ctx.subagentMapsRef, msgSessionId);
        if (upsertMessageInfo(subMap, info)) ctx.flushSubagentMessages();
        break;
      }

      let changed = upsertMessageInfo(ctx.messageMapRef.current, info);
      // A confirmed user message retires the placeholder standing in for it.
      // Retire after the upsert: a record that could not be stored must not
      // clear the only copy of the prompt on screen.
      if ((info.role as string) === "user" && reconcileOptimistic(ctx.messageMapRef.current)) {
        changed = true;
      }
      if (changed) ctx.flushMessages();

      // Update stats from the info (cost, tokens)
      if (info.cost !== undefined || info.tokens !== undefined) {
        const tokens = info.tokens as Record<string, unknown> | undefined;
        const cache = tokens?.cache as Record<string, number> | undefined;
        ctx.setStats((prev) => ({
          cost: (info.cost as number) ?? prev?.cost ?? 0,
          input_tokens: (tokens?.input as number) ?? prev?.input_tokens ?? 0,
          output_tokens: (tokens?.output as number) ?? prev?.output_tokens ?? 0,
          reasoning_tokens: (tokens?.reasoning as number) ?? prev?.reasoning_tokens ?? 0,
          cache_read: cache?.read ?? prev?.cache_read ?? 0,
          cache_write: cache?.write ?? prev?.cache_write ?? 0,
        }));
      }
      break;
    }

    case "message.part.updated": {
      const part = props.part as Record<string, unknown> | undefined;
      if (!part) break;
      const partSessionId = (part.sessionID as string) || "";

      if (isForeignSession(ctx, partSessionId)) {
        // Also update the session cache if this session is cached (background update)
        const cachedMap = getCachedMessageMap(ctx, partSessionId);
        if (cachedMap) upsertPart(cachedMap, part);

        const subMap = getOrCreateSubMap(ctx.subagentMapsRef, partSessionId);
        if (upsertPart(subMap, part)) ctx.flushSubagentMessages();
        break;
      }
      let partChanged = upsertPart(ctx.messageMapRef.current, part);
      // A user message's text arrives one event after its envelope, so this is the first
      // moment the placeholder can be matched on content rather than on clocks. Retiring
      // it here is what keeps the prompt from rendering twice.
      if (isUserMessagePart(ctx, part) && reconcileOptimistic(ctx.messageMapRef.current)) {
        partChanged = true;
      }
      if (partChanged) ctx.flushMessages();
      break;
    }

    case "message.part.delta": {
      const sessionID = (props.sessionID as string) || "";
      const messageID = (props.messageID as string) || "";
      const partID = (props.partID as string) || "";
      const field = (props.field as string) || "text";
      const delta = (props.delta as string) || "";
      if (!messageID || !partID || !delta) break;

      if (isForeignSession(ctx, sessionID)) {
        // Also update the session cache if this session is cached (background update)
        const cachedMap = getCachedMessageMap(ctx, sessionID);
        if (cachedMap) applyPartDelta(cachedMap, sessionID, messageID, partID, field, delta);

        const subMap = getOrCreateSubMap(ctx.subagentMapsRef, sessionID);
        if (applyPartDelta(subMap, sessionID, messageID, partID, field, delta)) ctx.flushSubagentMessages();
        break;
      }
      if (applyPartDelta(ctx.messageMapRef.current, sessionID, messageID, partID, field, delta)) ctx.flushMessages();
      break;
    }

    case "message.removed": {
      const msgId = (props.messageID as string) || "";
      const rmSessionId = (props.sessionID as string) || "";
      if (!msgId) break;

      if (isForeignSession(ctx, rmSessionId)) {
        // Update cached session if present
        const cached = getCachedSession(ctx, rmSessionId);
        if (cached && removeMessage(cached.messageMap, msgId)) {
          cached.totalMessageCount = Math.max(0, cached.totalMessageCount - 1);
        }
        // Also update subagent map
        const subMap = ctx.subagentMapsRef.current.get(rmSessionId);
        if (subMap && removeMessage(subMap, msgId)) ctx.flushSubagentMessages();
        break;
      }
      if (removeMessage(ctx.messageMapRef.current, msgId)) ctx.flushMessages();
      break;
    }

    case "message.part.removed": {
      const msgId = (props.messageID as string) || "";
      const partId = (props.partID as string) || "";
      const rpSessionId = (props.sessionID as string) || "";
      if (!msgId || !partId) break;

      if (isForeignSession(ctx, rpSessionId)) {
        // Update cached session if present
        const cachedMap = getCachedMessageMap(ctx, rpSessionId);
        if (cachedMap) removePart(cachedMap, msgId, partId);
        // Also update subagent map
        const subMap = ctx.subagentMapsRef.current.get(rpSessionId);
        if (subMap && removePart(subMap, msgId, partId)) ctx.flushSubagentMessages();
        break;
      }
      if (removePart(ctx.messageMapRef.current, msgId, partId)) ctx.flushMessages();
      break;
    }

    case "session.status": {
      const sid = props.sessionID as string | undefined;
      const rawStatus = props.status;

      // Parse into a full SessionStatus object (idle | busy | retry)
      let parsed: SessionStatus = SESSION_IDLE;
      if (rawStatus && typeof rawStatus === "object") {
        const obj = rawStatus as Record<string, unknown>;
        const t = obj.type as string | undefined;
        if (t === "busy") {
          parsed = { type: "busy" };
        } else if (t === "retry") {
          parsed = {
            type: "retry",
            attempt: typeof obj.attempt === "number" ? obj.attempt : 1,
            message: typeof obj.message === "string" ? obj.message : "Retrying…",
            next: typeof obj.next === "number" ? obj.next : Date.now() + 5000,
          };
        }
      } else if (typeof rawStatus === "string") {
        if (rawStatus === "busy") parsed = { type: "busy" };
        else if (rawStatus === "retry") parsed = { type: "retry", attempt: 1, message: "Retrying…", next: Date.now() + 5000 };
      }

      const isBusy = parsed.type !== "idle";

      // Update sessionStatuses map
      if (sid) {
        ctx.setSessionStatuses((prev) => {
          if (!isBusy) {
            if (!prev[sid]) return prev;
            const next = { ...prev };
            delete next[sid];
            return next;
          }
          const existing = prev[sid];
          // Avoid re-render if status hasn't meaningfully changed
          if (existing && existing.type === parsed.type) {
            if (parsed.type === "busy") return prev;
            if (parsed.type === "retry" && existing.type === "retry"
              && existing.attempt === parsed.attempt && existing.next === parsed.next) return prev;
          }
          return { ...prev, [sid]: parsed };
        });

        // Keep busySessions in sync
        if (isBusy) {
          ctx.setBusySessions((prev) => prev.has(sid) ? prev : new Set([...prev, sid]));
        } else {
          ctx.setBusySessions((prev) => { if (!prev.has(sid)) return prev; const next = new Set(prev); next.delete(sid); return next; });
        }
      }

      if (sid === ctx.activeSessionRef.current) {
        ctx.setSessionStatus(parsed);
      }
      break;
    }

    case "session.created":
      ctx.refreshState();
      break;

    case "session.updated": {
      // Update session metadata locally without a full refreshState() to avoid
      // active_session drift that causes unwanted session switches (e.g. when
      // selecting a model).
      const updatedInfo = props.info as Record<string, unknown> | undefined;
      if (updatedInfo) {
        ctx.updateSessionMeta(updatedInfo);
      }
      break;
    }

    case "session.deleted": {
      // Evict deleted session from the LRU cache
      const deletedSid = (props.sessionID ?? props.id ?? "") as string;
      if (deletedSid) ctx.sessionCacheRef.current.delete(deletedSid);
      ctx.refreshState();
      break;
    }

    case "permission.asked": {
      const perm: PermissionRequest = {
        id: (props.id ?? props.requestID ?? "") as string,
        sessionID: (props.sessionID ?? "") as string,
        toolName: (props.permission ?? props.toolName ?? "") as string,
        description: formatPermissionDescription(props),
        patterns: Array.isArray(props.patterns) ? (props.patterns as string[]) : undefined,
        metadata: (props.metadata && typeof props.metadata === "object")
          ? props.metadata as Record<string, unknown> : undefined,
        time: Date.now(),
      };
      if (perm.id) {
        if (perm.sessionID === ctx.activeSessionRef.current) {
          ctx.setPermissions((prev) => [...prev.filter((p) => p.id !== perm.id), perm]);
        } else {
          ctx.setCrossSessionPermissions((prev) => [...prev.filter((p) => p.id !== perm.id), perm]);
        }
      }
      break;
    }

    case "permission.replied": {
      const requestID = (props.requestID ?? props.id ?? "") as string;
      if (requestID) {
        ctx.setPermissions((prev) => prev.filter((p) => p.id !== requestID));
        ctx.setCrossSessionPermissions((prev) => prev.filter((p) => p.id !== requestID));
      }
      break;
    }

    case "question.asked": {
      const rawQuestions = Array.isArray(props.questions) ? props.questions : [];
      const q: QuestionRequest = {
        id: (props.id ?? props.requestID ?? "") as string,
        sessionID: (props.sessionID ?? "") as string,
        title: deriveQuestionTitle(props, rawQuestions),
        questions: rawQuestions.map(transformQuestionInfo),
        time: Date.now(),
      };
      if (q.id && !ctx.resolvedQuestionIdsRef.current.has(q.id)) {
        if (q.sessionID === ctx.activeSessionRef.current) {
          ctx.setQuestions((prev) => [...prev.filter((qp) => qp.id !== q.id), q]);
        } else {
          ctx.setCrossSessionQuestions((prev) => [...prev.filter((qp) => qp.id !== q.id), q]);
        }
      }
      break;
    }

    case "question.replied":
    case "question.rejected": {
      const requestID = (props.requestID ?? props.id ?? "") as string;
      if (requestID) {
        // Record as resolved so a later hydrate/re-ask can't bring it back.
        const resolved = ctx.resolvedQuestionIdsRef.current;
        resolved.add(requestID);
        if (resolved.size > 500) resolved.delete(resolved.values().next().value as string);
        ctx.setQuestions((prev) => prev.filter((q) => q.id !== requestID));
        ctx.setCrossSessionQuestions((prev) => prev.filter((q) => q.id !== requestID));
      }
      break;
    }

    case "todo.updated":
      window.dispatchEvent(new CustomEvent("opman:todo-updated", {
        detail: { sessionID: (props.sessionID as string) || "" },
      }));
      break;
    case "session.queue":
      window.dispatchEvent(new CustomEvent("opman:queue-updated", {
        detail: {
          sessionID: (props.sessionID as string) || "",
          pending: Array.isArray(props.pending) ? (props.pending as string[]) : [],
        },
      }));
      break;
    case "file.edited": {
      const editSessionId = (props.sessionID as string) || "";
      // Only increment for the active session (or if no sessionID is provided for backward compat)
      if (!editSessionId || editSessionId === ctx.activeSessionRef.current) {
        ctx.setFileEditCount((prev) => prev + 1);
      }
      break;
    }
  }
}
/** Setters for app-level SSE events (used by setupAppSSE). */
export interface AppSSEContext {
  activeSessionRef: { current: string | null };
  sessionCacheRef: { current: Map<string, CachedSession> };
  refreshState: () => void;
  touchEvent: () => void;
  recoverAfterReconnect: () => void;
  setBusySessions: React.Dispatch<React.SetStateAction<Set<string>>>;
  setSessionStatus: React.Dispatch<React.SetStateAction<SessionStatus>>;
  setSessionStatuses: React.Dispatch<React.SetStateAction<Record<string, SessionStatus>>>;
  setStats: React.Dispatch<React.SetStateAction<SessionStats | null>>;
  setWatcherStatus: React.Dispatch<React.SetStateAction<WatcherStatus | null>>;
  setMcpEditorOpenPath: React.Dispatch<React.SetStateAction<string | null>>;
  setMcpEditorOpenLine: React.Dispatch<React.SetStateAction<number | null>>;
  setMcpTerminalFocusId: React.Dispatch<React.SetStateAction<string | null>>;
  setMcpAgentActivity: React.Dispatch<React.SetStateAction<Map<string, boolean>>>;
  setPresenceClients: React.Dispatch<React.SetStateAction<ClientPresence[]>>;
}

/** Wire all listeners onto the app-level EventSource.
 *  Uses addEventListener instead of onopen/onerror so callers can safely
 *  add additional open/error listeners without overwriting these. */
export function setupAppSSEListeners(appSSE: EventSource, ctx: AppSSEContext): void {
  let needsRecovery = false;
  appSSE.addEventListener("heartbeat", () => { ctx.touchEvent(); });
  appSSE.addEventListener("error", () => {
    console.warn("[SSE] App events connection error — EventSource will auto-reconnect");
    needsRecovery = true;
  });
  appSSE.addEventListener("open", () => {
    ctx.touchEvent();
    if (needsRecovery) {
      needsRecovery = false;
      ctx.recoverAfterReconnect();
    }
  });

  appSSE.addEventListener("state_changed", () => { ctx.touchEvent(); ctx.refreshState(); });
  appSSE.addEventListener("session_busy", (e: MessageEvent) => {
    const sid = e.data;
    ctx.setBusySessions((prev) => prev.has(sid) ? prev : new Set([...prev, sid]));
    ctx.setSessionStatuses((prev) => {
      if (prev[sid]?.type === "busy") return prev;
      return { ...prev, [sid]: { type: "busy" } };
    });
    if (sid === ctx.activeSessionRef.current) ctx.setSessionStatus({ type: "busy" });
  });
  appSSE.addEventListener("session_idle", (e: MessageEvent) => {
    const sid = e.data;
    ctx.setBusySessions((prev) => { if (!prev.has(sid)) return prev; const next = new Set(prev); next.delete(sid); return next; });
    ctx.setSessionStatuses((prev) => {
      if (!prev[sid]) return prev;
      const next = { ...prev };
      delete next[sid];
      return next;
    });
    if (sid === ctx.activeSessionRef.current) ctx.setSessionStatus(SESSION_IDLE);
  });
  appSSE.addEventListener("stats_updated", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as SessionStats;
      const statsSid = data.session_id || "";
      // If the stats belong to the active session (or no session_id for backward compat), update display
      if (!statsSid || statsSid === ctx.activeSessionRef.current) {
        ctx.setStats(data);
      } else {
        // Update cached session stats in the background
        const cached = ctx.sessionCacheRef.current.get(statsSid);
        if (cached) cached.stats = data;
      }
    } catch { /* ignore */ }
  });
  appSSE.addEventListener("theme_changed", (e: MessageEvent) => {
    try {
      const pair: ThemePair = JSON.parse(e.data);
      const appearance = getPersistedAppearance();
      storeThemePair(pair, appearance);
      applyThemeToCss(resolveThemeColors(pair, appearance));
    } catch { /* ignore */ }
  });
  appSSE.addEventListener("watcher_status", (e: MessageEvent) => {
    try {
      const status = JSON.parse(e.data) as WatcherStatus;
      ctx.setWatcherStatus(status.action === "deleted" ? null : status);
    } catch { /* ignore */ }
  });

  // MCP events
  appSSE.addEventListener("mcp_editor_open", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as McpEditorOpen;
      ctx.setMcpEditorOpenPath(data.path);
      ctx.setMcpEditorOpenLine(data.line);
    } catch { /* ignore */ }
  });
  appSSE.addEventListener("mcp_editor_navigate", (e: MessageEvent) => {
    try { ctx.setMcpEditorOpenLine(JSON.parse(e.data).line ?? null); } catch { /* ignore */ }
  });
  appSSE.addEventListener("mcp_terminal_focus", (e: MessageEvent) => {
    ctx.setMcpTerminalFocusId(e.data);
  });
  appSSE.addEventListener("mcp_agent_activity", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as McpAgentActivity;
      ctx.setMcpAgentActivity((prev) => {
        if (data.active) {
          if (prev.has(data.tool)) return prev; // no change
          const next = new Map(prev);
          next.set(data.tool, true);
          return next;
        } else {
          if (!prev.has(data.tool)) return prev; // no change
          const next = new Map(prev);
          next.delete(data.tool);
          return next;
        }
      });
    } catch { /* ignore */ }
  });

  // Presence
  appSSE.addEventListener("presence_changed", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as { clients: ClientPresence[] };
      ctx.setPresenceClients(data.clients);
    } catch { /* ignore */ }
  });

  // Routine updates
  appSSE.addEventListener("routine_updated", () => {
    window.dispatchEvent(new CustomEvent("opman:routine-updated"));
  });

  // Toast notifications from TUI
  appSSE.addEventListener("toast", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as { message: string; level: string };
      window.dispatchEvent(new CustomEvent("opman:toast", { detail: data }));
    } catch { /* ignore */ }
  });
}
