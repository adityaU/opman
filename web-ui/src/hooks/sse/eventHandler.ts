import type { PermissionRequest, QuestionRequest, OpenCodeEvent } from "../../types";
import type { SessionStats, ActivityEvent, ClientPresence } from "../../api";
import { applyThemeToCss } from "../../utils/theme";
import type { WatcherStatus, McpAgentActivity, McpEditorOpen } from "./types";
import { formatPermissionDescription, deriveQuestionTitle, transformQuestionInfo } from "./transforms";
import { type MessageMap, upsertMessageInfo, upsertPart, applyPartDelta, removeMessage, removePart } from "./messageMap";

/** Setters and refs needed by the event handler — avoids passing 20+ individual args. */
export interface EventHandlerContext {
  activeSessionRef: { current: string | null };
  messageMapRef: { current: MessageMap };
  subagentMapsRef: { current: Map<string, MessageMap> };
  flushMessages: () => void;
  flushSubagentMessages: () => void;
  refreshState: () => void;
  setStats: React.Dispatch<React.SetStateAction<SessionStats | null>>;
  setSessionStatus: React.Dispatch<React.SetStateAction<"idle" | "busy">>;
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
        const subMap = getOrCreateSubMap(ctx.subagentMapsRef, sessionID);
        if (applyPartDelta(subMap, sessionID, messageID, partID, field, delta)) ctx.flushSubagentMessages();
        break;
      }
      if (applyPartDelta(ctx.messageMapRef.current, sessionID, messageID, partID, field, delta)) ctx.flushMessages();
      break;
    }

    case "message.removed": {
      const msgId = (props.messageID as string) || "";
      if (msgId && removeMessage(ctx.messageMapRef.current, msgId)) ctx.flushMessages();
      break;
    }

    case "message.part.removed": {
      const msgId = (props.messageID as string) || "";
      const partId = (props.partID as string) || "";
      if (msgId && partId && removePart(ctx.messageMapRef.current, msgId, partId)) ctx.flushMessages();
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
      if (sid === ctx.activeSessionRef.current) {
        ctx.setSessionStatus(statusStr === "busy" || statusStr === "retry" ? "busy" : "idle");
      }
      break;
    }

    case "session.created":
    case "session.updated":
    case "session.deleted":
      ctx.refreshState();
      break;

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

    case "todo.updated": break;
    case "file.edited": ctx.setFileEditCount((prev) => prev + 1); break;
  }
}
/** Setters for app-level SSE events (used by setupAppSSE). */
export interface AppSSEContext {
  activeSessionRef: { current: string | null };
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

/** Wire all listeners onto the app-level EventSource. */
export function setupAppSSEListeners(appSSE: EventSource, ctx: AppSSEContext): void {
  appSSE.addEventListener("heartbeat", () => { ctx.touchEvent(); });
  appSSE.onerror = () => {
    console.warn("[SSE] App events connection error — EventSource will auto-reconnect");
    (appSSE as unknown as { _needsRecovery: boolean })._needsRecovery = true;
  };
  appSSE.onopen = () => {
    ctx.touchEvent();
    if ((appSSE as unknown as { _needsRecovery?: boolean })._needsRecovery) {
      (appSSE as unknown as { _needsRecovery: boolean })._needsRecovery = false;
      ctx.recoverAfterReconnect();
    }
  };

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
    try { ctx.setStats(JSON.parse(e.data)); } catch { /* ignore */ }
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
      }
    } catch { /* ignore */ }
  });
}
