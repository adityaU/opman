import type { PermissionRequest, QuestionRequest, OpenCodeEvent } from "../../types";
import type { SessionStats, ActivityEvent, ClientPresence, Mission } from "../../api";
import { applyThemeToCss } from "../../utils/theme";
import type { WatcherStatus, McpAgentActivity, McpEditorOpen } from "./types";
import type { CachedSession } from "./useSSE";
import { formatPermissionDescription, deriveQuestionTitle, transformQuestionInfo } from "./transforms";
import { type MessageMap, upsertMessageInfo, upsertPart, applyPartDelta, removeMessage, removePart } from "./messageMap";

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
  setStats: React.Dispatch<React.SetStateAction<SessionStats | null>>;
  setSessionStatus: React.Dispatch<React.SetStateAction<"idle" | "busy">>;
  setBusySessions: React.Dispatch<React.SetStateAction<Set<string>>>;
  setPermissions: React.Dispatch<React.SetStateAction<PermissionRequest[]>>;
  setQuestions: React.Dispatch<React.SetStateAction<QuestionRequest[]>>;
  setCrossSessionPermissions: React.Dispatch<React.SetStateAction<PermissionRequest[]>>;
  setCrossSessionQuestions: React.Dispatch<React.SetStateAction<QuestionRequest[]>>;
  setFileEditCount: React.Dispatch<React.SetStateAction<number>>;
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

/** Route an opencode SSE event to the appropriate React state updaters. */
export function handleOpenCodeEvent(ctx: EventHandlerContext, event: OpenCodeEvent): void {
  const props = event.properties || {};

  switch (event.type) {
    case "message.updated": {
      const info = props.info as Record<string, unknown> | undefined;
      if (!info) break;
      const msgSessionId = (info.sessionID as string) || "";

      // Route to subagent map if not the active session
      if (ctx.activeSessionRef.current && msgSessionId && msgSessionId !== ctx.activeSessionRef.current) {
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

      if (upsertMessageInfo(ctx.messageMapRef.current, info)) ctx.flushMessages();

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

      if (ctx.activeSessionRef.current && partSessionId && partSessionId !== ctx.activeSessionRef.current) {
        // Also update the session cache if this session is cached (background update)
        const cachedMap = getCachedMessageMap(ctx, partSessionId);
        if (cachedMap) upsertPart(cachedMap, part);

        const subMap = getOrCreateSubMap(ctx.subagentMapsRef, partSessionId);
        if (upsertPart(subMap, part)) ctx.flushSubagentMessages();
        break;
      }
      if (upsertPart(ctx.messageMapRef.current, part)) ctx.flushMessages();
      break;
    }

    case "message.part.delta": {
      const sessionID = (props.sessionID as string) || "";
      const messageID = (props.messageID as string) || "";
      const partID = (props.partID as string) || "";
      const field = (props.field as string) || "text";
      const delta = (props.delta as string) || "";
      if (!messageID || !partID || !delta) break;

      if (ctx.activeSessionRef.current && sessionID && sessionID !== ctx.activeSessionRef.current) {
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

      if (ctx.activeSessionRef.current && rmSessionId && rmSessionId !== ctx.activeSessionRef.current) {
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

      if (ctx.activeSessionRef.current && rpSessionId && rpSessionId !== ctx.activeSessionRef.current) {
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
      let statusStr: string | undefined;
      if (typeof rawStatus === "string") statusStr = rawStatus;
      else if (rawStatus && typeof rawStatus === "object") {
        statusStr = (rawStatus as Record<string, unknown>).type as string | undefined;
      }
      const isBusy = statusStr === "busy" || statusStr === "retry";
      // Keep busySessions in sync (mirrors the app-level SSE path)
      if (sid) {
        if (isBusy) {
          ctx.setBusySessions((prev) => new Set([...prev, sid]));
        } else {
          ctx.setBusySessions((prev) => { const next = new Set(prev); next.delete(sid); return next; });
        }
      }
      if (sid === ctx.activeSessionRef.current) {
        ctx.setSessionStatus(isBusy ? "busy" : "idle");
      }
      break;
    }

    case "session.created":
    case "session.updated":
      ctx.refreshState();
      break;

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
      if (q.id) {
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
  setSessionStatus: React.Dispatch<React.SetStateAction<"idle" | "busy">>;
  setStats: React.Dispatch<React.SetStateAction<SessionStats | null>>;
  setWatcherStatus: React.Dispatch<React.SetStateAction<WatcherStatus | null>>;
  setMcpEditorOpenPath: React.Dispatch<React.SetStateAction<string | null>>;
  setMcpEditorOpenLine: React.Dispatch<React.SetStateAction<number | null>>;
  setMcpTerminalFocusId: React.Dispatch<React.SetStateAction<string | null>>;
  setMcpAgentActivity: React.Dispatch<React.SetStateAction<Map<string, boolean>>>;
  setPresenceClients: React.Dispatch<React.SetStateAction<ClientPresence[]>>;
  setLiveActivityEvents: React.Dispatch<React.SetStateAction<ActivityEvent[]>>;
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
    ctx.setBusySessions((prev) => new Set([...prev, e.data]));
    if (e.data === ctx.activeSessionRef.current) ctx.setSessionStatus("busy");
  });
  appSSE.addEventListener("session_idle", (e: MessageEvent) => {
    ctx.setBusySessions((prev) => { const next = new Set(prev); next.delete(e.data); return next; });
    if (e.data === ctx.activeSessionRef.current) ctx.setSessionStatus("idle");
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
    try { applyThemeToCss(JSON.parse(e.data)); } catch { /* ignore */ }
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
        const next = new Map(prev);
        if (data.active) next.set(data.tool, true); else next.delete(data.tool);
        return next;
      });
    } catch { /* ignore */ }
  });

  // Presence + activity
  appSSE.addEventListener("presence_changed", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as { clients: ClientPresence[] };
      ctx.setPresenceClients(data.clients);
    } catch { /* ignore */ }
  });
  appSSE.addEventListener("activity_event", (e: MessageEvent) => {
    try {
      const data = JSON.parse(e.data) as ActivityEvent;
      const currentSession = ctx.activeSessionRef.current;
      if (currentSession && data.session_id === currentSession) {
        ctx.setLiveActivityEvents((prev) => {
          const next = [...prev, data];
          return next.length > 200 ? next.slice(-200) : next;
        });
      } else if (data.session_id) {
        // Append to cached session's activity events (background update)
        const cached = ctx.sessionCacheRef.current.get(data.session_id);
        if (cached) {
          cached.liveActivityEvents = [...cached.liveActivityEvents, data];
          if (cached.liveActivityEvents.length > 200) {
            cached.liveActivityEvents = cached.liveActivityEvents.slice(-200);
          }
        }
      }
    } catch { /* ignore */ }
  });

  // Mission loop updates
  appSSE.addEventListener("mission_updated", (e: MessageEvent) => {
    try {
      const mission = JSON.parse(e.data) as Mission;
      window.dispatchEvent(new CustomEvent("opman:mission-updated", { detail: mission }));
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
