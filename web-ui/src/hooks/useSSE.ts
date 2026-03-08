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
  type ActivityEvent,
  type ClientPresence,
} from "../api";
import type { Message, MessagePart, PermissionRequest, QuestionRequest, OpenCodeEvent } from "../types";
import { applyThemeToCss } from "../utils/theme";

/** Watcher status pushed via SSE (mirrors backend WatcherStatusEvent). */
export interface WatcherStatus {
  session_id: string;
  /** "created" | "deleted" | "triggered" | "countdown" | "cancelled" */
  action: string;
  idle_since_secs: number | null;
}

/** MCP agent activity event — indicates an AI agent is using a tool. */
export interface McpAgentActivity {
  tool: string;
  active: boolean;
}

/** MCP editor open event — AI agent requests a file be opened. */
export interface McpEditorOpen {
  path: string;
  line: number | null;
}

export interface SSEState {
  appState: AppState | null;
  messages: Message[];
  stats: SessionStats | null;
  busySessions: Set<string>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  sessionStatus: "idle" | "busy";
  /** True while loading messages for a newly-selected session */
  isLoadingMessages: boolean;
  /** Latest watcher status from SSE (null = no watcher or deleted). */
  watcherStatus: WatcherStatus | null;
  /** Messages for subagent sessions, keyed by session ID. */
  subagentMessages: Map<string, Message[]>;
  /** Counter incremented on each file.edited SSE event — triggers diff panel refresh. */
  fileEditCount: number;
  /** MCP: file path the AI agent wants to open in the editor. */
  mcpEditorOpenPath: string | null;
  /** MCP: line number to navigate to (set with mcpEditorOpenPath). */
  mcpEditorOpenLine: number | null;
  /** MCP: terminal ID the AI agent wants to focus. */
  mcpTerminalFocusId: string | null;
  /** MCP: currently active agent tools (tool name → true). */
  mcpAgentActivity: Map<string, boolean>;
  /** Connected clients (presence tracking). */
  presenceClients: ClientPresence[];
  /** Live activity events for the active session (newest last). */
  liveActivityEvents: ActivityEvent[];
  refreshState: () => Promise<void>;
  refreshMessages: () => Promise<void>;
  clearPermission: (id: string) => void;
  clearQuestion: (id: string) => void;
  /** Clear MCP editor open request (after frontend has handled it). */
  clearMcpEditorOpen: () => void;
  /** Clear MCP terminal focus request (after frontend has handled it). */
  clearMcpTerminalFocus: () => void;
  /** Add an optimistic user message that shows immediately.
   *  It will be removed when the real server message arrives via refreshMessages/SSE. */
  addOptimisticMessage: (text: string) => void;
}

// ── Helpers for extracting message ID ──────────────────────────────

/** Get the canonical message ID from a Message's info field.
 *  REST responses use `messageID`, SSE events use `id`. */
function getMessageId(msg: Message): string {
  return msg.info.messageID || msg.info.id || "";
}

/** Get the creation timestamp from a message for sorting. */
function getMessageTime(msg: Message): number {
  const t = msg.info.time;
  if (typeof t === "number") return t;
  if (t && typeof t === "object") return t.created ?? 0;
  // Also check metadata.time which REST responses include
  if (msg.metadata?.time?.created) return msg.metadata.time.created;
  return 0;
}

// ── Message Map management ─────────────────────────────────────────

/**
 * In-memory message store keyed by message ID.
 * This allows O(1) upserts from SSE events instead of re-fetching all messages.
 */
type MessageMap = Map<string, Message>;

/** Convert a MessageMap to a sorted array for rendering. */
function mapToSortedArray(map: MessageMap): Message[] {
  return Array.from(map.values()).sort(
    (a, b) => getMessageTime(a) - getMessageTime(b),
  );
}

/** Upsert message info from a `message.updated` SSE event.
 *  Merges new info fields into the existing message if it exists,
 *  preserving the parts array.  Creates a new message entry if not. */
function upsertMessageInfo(map: MessageMap, info: Record<string, unknown>): boolean {
  // SSE events use `id`, REST uses `messageID`
  const msgId = (info.id as string) || (info.messageID as string);
  if (!msgId) return false;

  const existing = map.get(msgId);
  if (existing) {
    // Merge info fields — keep existing parts
    const merged: Message = {
      ...existing,
      info: { ...existing.info, ...info, messageID: msgId },
      metadata: {
        ...existing.metadata,
        cost: (info.cost as number) ?? existing.metadata?.cost,
        time: info.time
          ? typeof info.time === "object"
            ? (info.time as { created?: number; completed?: number })
            : { created: info.time as number }
          : existing.metadata?.time,
        tokens: info.tokens
          ? (() => {
              const t = info.tokens as Record<string, unknown>;
              const cache = t.cache as Record<string, number> | undefined;
              return {
                input: (t.input as number) ?? 0,
                output: (t.output as number) ?? 0,
                reasoning: (t.reasoning as number) ?? 0,
                cache_read: cache?.read ?? 0,
                cache_write: cache?.write ?? 0,
              };
            })()
          : existing.metadata?.tokens,
      },
    };
    map.set(msgId, merged);
    return true;
  }

  // New message — create with empty parts (parts come via message.part.updated)
  const role = (info.role as string) || "assistant";
  const newMsg: Message = {
    info: {
      ...(info as unknown as Message["info"]),
      role: role as Message["info"]["role"],
      messageID: msgId,
    },
    parts: [],
    metadata: {
      cost: (info.cost as number) ?? 0,
      time: info.time
        ? typeof info.time === "object"
          ? (info.time as { created?: number; completed?: number })
          : { created: info.time as number }
        : undefined,
      tokens: info.tokens
        ? (() => {
            const t = info.tokens as Record<string, unknown>;
            const cache = t.cache as Record<string, number> | undefined;
            return {
              input: (t.input as number) ?? 0,
              output: (t.output as number) ?? 0,
              reasoning: (t.reasoning as number) ?? 0,
              cache_read: cache?.read ?? 0,
              cache_write: cache?.write ?? 0,
            };
          })()
        : undefined,
    },
  };
  map.set(msgId, newMsg);
  return true;
}

/** Upsert a part from a `message.part.updated` SSE event.
 *  Finds the parent message and adds/replaces the part by its `id`. */
function upsertPart(map: MessageMap, part: Record<string, unknown>): boolean {
  const msgId = part.messageID as string;
  const partId = part.id as string;
  if (!msgId || !partId) return false;

  let msg = map.get(msgId);
  if (!msg) {
    // The part arrived before the message.updated event — create a skeleton
    msg = {
      info: {
        role: "assistant",
        messageID: msgId,
        sessionID: part.sessionID as string,
      },
      parts: [],
    };
    map.set(msgId, msg);
  }

  // Find existing part by id and replace, or append
  const existingIdx = msg.parts.findIndex((p) => p.id === partId);
  const newPart = part as unknown as MessagePart;
  if (existingIdx >= 0) {
    msg.parts[existingIdx] = newPart;
  } else {
    msg.parts.push(newPart);
  }

  // Must create a new message object reference for React to detect change
  map.set(msgId, { ...msg, parts: [...msg.parts] });
  return true;
}

/** Apply a text delta from a `message.part.delta` SSE event.
 *  Appends the delta string to the specified field of the part. */
function applyPartDelta(
  map: MessageMap,
  sessionID: string,
  messageID: string,
  partID: string,
  field: string,
  delta: string,
): boolean {
  const msg = map.get(messageID);
  if (!msg) return false;

  const part = msg.parts.find((p) => p.id === partID);
  if (!part) {
    // Part not yet in the message — create a placeholder text part
    const newPart: MessagePart = {
      type: "text",
      id: partID,
      sessionID,
      messageID,
      [field]: delta,
    };
    msg.parts.push(newPart);
    map.set(messageID, { ...msg, parts: [...msg.parts] });
    return true;
  }

  // Append delta to the field (usually "text")
  const current = (part as unknown as Record<string, unknown>)[field];
  (part as unknown as Record<string, unknown>)[field] =
    typeof current === "string" ? current + delta : delta;

  // Create new references for React change detection
  map.set(messageID, { ...msg, parts: [...msg.parts] });
  return true;
}

/** Remove a message from the map. */
function removeMessage(map: MessageMap, messageID: string): boolean {
  return map.delete(messageID);
}

/** Remove a part from a message. */
function removePart(map: MessageMap, messageID: string, partID: string): boolean {
  const msg = map.get(messageID);
  if (!msg) return false;

  const filtered = msg.parts.filter((p) => p.id !== partID);
  if (filtered.length === msg.parts.length) return false; // wasn't there

  map.set(messageID, { ...msg, parts: filtered });
  return true;
}

// ── Hook ───────────────────────────────────────────────────────────

export function useSSE(): SSEState {
  const [appState, setAppState] = useState<AppState | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [stats, setStats] = useState<SessionStats | null>(null);
  const [busySessions, setBusySessions] = useState<Set<string>>(new Set());
  const [permissions, setPermissions] = useState<PermissionRequest[]>([]);
  const [questions, setQuestions] = useState<QuestionRequest[]>([]);
  const [sessionStatus, setSessionStatus] = useState<"idle" | "busy">("idle");
  const [isLoadingMessages, setIsLoadingMessages] = useState(false);
  const [watcherStatus, setWatcherStatus] = useState<WatcherStatus | null>(null);
  const [subagentMessages, setSubagentMessages] = useState<Map<string, Message[]>>(new Map());
  const [fileEditCount, setFileEditCount] = useState(0);
  const [mcpEditorOpenPath, setMcpEditorOpenPath] = useState<string | null>(null);
  const [mcpEditorOpenLine, setMcpEditorOpenLine] = useState<number | null>(null);
  const [mcpTerminalFocusId, setMcpTerminalFocusId] = useState<string | null>(null);
  const [mcpAgentActivity, setMcpAgentActivity] = useState<Map<string, boolean>>(new Map());
  const [presenceClients, setPresenceClients] = useState<ClientPresence[]>([]);
  const [liveActivityEvents, setLiveActivityEvents] = useState<ActivityEvent[]>([]);
  const activeSessionRef = useRef<string | null>(null);

  /**
   * Generation counter — incremented every time the active session changes.
   * Used to discard stale fetch results from a previous session.
   */
  const sessionGenRef = useRef(0);

  /** In-memory message map for efficient SSE-driven updates. */
  const messageMapRef = useRef<MessageMap>(new Map());

  /** In-memory message maps for subagent sessions, keyed by session ID. */
  const subagentMapsRef = useRef<Map<string, MessageMap>>(new Map());

  /**
   * Flush the message map to React state.
   * Called after SSE events modify the map, batched with a microtask
   * to coalesce rapid-fire events (e.g. many part.delta events in a row).
   */
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushMessages = useCallback(() => {
    // Debounce — coalesce rapid SSE events into a single React render
    if (flushTimerRef.current) return;
    flushTimerRef.current = setTimeout(() => {
      flushTimerRef.current = null;
      const gen = sessionGenRef.current;
      // Only flush if the session hasn't changed
      if (gen === sessionGenRef.current) {
        setMessages(mapToSortedArray(messageMapRef.current));
      }
    }, 16); // ~1 frame at 60fps
  }, []);

  /**
   * Flush subagent message maps to React state.
   * Uses the same debounce pattern as main messages.
   */
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

  /**
   * Fetch the latest app state from the server.
   */
  const refreshState = useCallback(async () => {
    try {
      const s = await fetchAppState();
      setAppState(s);
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

  /** No-op — messages are fully SSE-driven. Kept for interface compat. */
  const refreshMessages = useCallback(async () => {}, []);

  const clearPermission = useCallback((id: string) => {
    setPermissions((prev) => prev.filter((p) => p.id !== id));
  }, []);

  const clearQuestion = useCallback((id: string) => {
    setQuestions((prev) => prev.filter((q) => q.id !== id));
  }, []);

  // Track active session changes — fetch history once, then SSE keeps it up to date.
  useEffect(() => {
    if (!appState) return;
    const proj = appState.projects[appState.active_project];
    const sid = proj?.active_session ?? null;
    if (sid !== activeSessionRef.current) {
      sessionGenRef.current += 1;
      const gen = sessionGenRef.current;
      activeSessionRef.current = sid;

      // Clear the message map for the new session
      messageMapRef.current = new Map();
      setMessages([]);
      // Clear live activity events for the new session
      setLiveActivityEvents([]);

      if (sid) {
        setIsLoadingMessages(true);

        // Initial REST fetch to populate message history
        fetchSessionMessages(sid)
          .then((msgs) => {
            if (gen !== sessionGenRef.current) return; // stale
            const newMap: MessageMap = new Map();
            for (const msg of msgs) {
              const id = msg.info.messageID || msg.info.id || "";
              if (id) newMap.set(id, msg);
            }
            messageMapRef.current = newMap;
            setMessages(mapToSortedArray(newMap));
          })
          .catch(() => {
            if (gen !== sessionGenRef.current) return;
            setMessages([]);
          })
          .finally(() => {
            if (gen !== sessionGenRef.current) return;
            setIsLoadingMessages(false);
          });

        // Fetch stats for the new session
        fetchSessionStats(sid)
          .then((st) => {
            if (gen !== sessionGenRef.current) return;
            setStats(st);
          })
          .catch(() => {});
      } else {
        setIsLoadingMessages(false);
      }
    }
  }, [appState]);

  // SSE connections — set up once on mount
  useEffect(() => {
    // Initial load: fetch app state + theme
    refreshState();
    fetchTheme().then((colors) => {
      if (colors) applyThemeToCss(colors);
    });

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
    appSSE.addEventListener("watcher_status", (e: MessageEvent) => {
      try {
        const status = JSON.parse(e.data) as WatcherStatus;
        if (status.action === "deleted") {
          setWatcherStatus(null);
        } else {
          setWatcherStatus(status);
        }
      } catch { /* ignore */ }
    });

    // MCP events — AI agent controlling the web UI
    appSSE.addEventListener("mcp_editor_open", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as McpEditorOpen;
        setMcpEditorOpenPath(data.path);
        setMcpEditorOpenLine(data.line);
      } catch { /* ignore */ }
    });
    appSSE.addEventListener("mcp_editor_navigate", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        // Navigate triggers a re-open at the new line
        setMcpEditorOpenLine(data.line ?? null);
      } catch { /* ignore */ }
    });
    appSSE.addEventListener("mcp_terminal_focus", (e: MessageEvent) => {
      setMcpTerminalFocusId(e.data);
    });
    appSSE.addEventListener("mcp_agent_activity", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as McpAgentActivity;
        setMcpAgentActivity((prev) => {
          const next = new Map(prev);
          if (data.active) {
            next.set(data.tool, true);
          } else {
            next.delete(data.tool);
          }
          return next;
        });
      } catch { /* ignore */ }
    });

    // Session Continuity: presence + activity events
    appSSE.addEventListener("presence_changed", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as { clients: ClientPresence[] };
        setPresenceClients(data.clients);
      } catch { /* ignore */ }
    });
    appSSE.addEventListener("activity_event", (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as ActivityEvent;
        // Only keep events for the current active session
        const currentSession = activeSessionRef.current;
        if (currentSession && data.session_id === currentSession) {
          setLiveActivityEvents((prev) => {
            const next = [...prev, data];
            // Keep last 200
            if (next.length > 200) return next.slice(-200);
            return next;
          });
        }
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
        // ── Message info update ────────────────────────────
        case "message.updated": {
          const info = props.info as Record<string, unknown> | undefined;
          if (!info) break;
          const msgSessionId = (info.sessionID as string) || "";

          // Route to subagent map if this is not the active session
          if (activeSessionRef.current && msgSessionId && msgSessionId !== activeSessionRef.current) {
            let subMap = subagentMapsRef.current.get(msgSessionId);
            if (!subMap) {
              subMap = new Map();
              subagentMapsRef.current.set(msgSessionId, subMap);
            }
            const changed = upsertMessageInfo(subMap, info);
            if (changed) flushSubagentMessages();
            break;
          }

          const changed = upsertMessageInfo(messageMapRef.current, info);
          if (changed) flushMessages();

          // Also update stats from the info (cost, tokens)
          if (info.cost !== undefined || info.tokens !== undefined) {
            const tokens = info.tokens as Record<string, unknown> | undefined;
            const cache = tokens?.cache as Record<string, number> | undefined;
            setStats((prev) => ({
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

        // ── Part update (full part object) ─────────────────
        case "message.part.updated": {
          const part = props.part as Record<string, unknown> | undefined;
          if (!part) break;
          const partSessionId = (part.sessionID as string) || "";

          // Route to subagent map if this is not the active session
          if (activeSessionRef.current && partSessionId && partSessionId !== activeSessionRef.current) {
            let subMap = subagentMapsRef.current.get(partSessionId);
            if (!subMap) {
              subMap = new Map();
              subagentMapsRef.current.set(partSessionId, subMap);
            }
            const changed = upsertPart(subMap, part);
            if (changed) flushSubagentMessages();
            break;
          }

          const changed = upsertPart(messageMapRef.current, part);
          if (changed) flushMessages();
          break;
        }

        // ── Part text delta (streaming) ────────────────────
        case "message.part.delta": {
          const sessionID = (props.sessionID as string) || "";
          const messageID = (props.messageID as string) || "";
          const partID = (props.partID as string) || "";
          const field = (props.field as string) || "text";
          const delta = (props.delta as string) || "";
          if (!messageID || !partID || !delta) break;

          // Route to subagent map if this is not the active session
          if (activeSessionRef.current && sessionID && sessionID !== activeSessionRef.current) {
            let subMap = subagentMapsRef.current.get(sessionID);
            if (!subMap) {
              subMap = new Map();
              subagentMapsRef.current.set(sessionID, subMap);
            }
            const changed = applyPartDelta(subMap, sessionID, messageID, partID, field, delta);
            if (changed) flushSubagentMessages();
            break;
          }

          const changed = applyPartDelta(
            messageMapRef.current,
            sessionID,
            messageID,
            partID,
            field,
            delta,
          );
          if (changed) flushMessages();
          break;
        }

        // ── Message removed ────────────────────────────────
        case "message.removed": {
          const msgId = (props.messageID as string) || "";
          if (!msgId) break;
          const changed = removeMessage(messageMapRef.current, msgId);
          if (changed) flushMessages();
          break;
        }

        // ── Part removed ───────────────────────────────────
        case "message.part.removed": {
          const msgId = (props.messageID as string) || "";
          const partId = (props.partID as string) || "";
          if (!msgId || !partId) break;
          const changed = removePart(messageMapRef.current, msgId, partId);
          if (changed) flushMessages();
          break;
        }

        // ── Session status ─────────────────────────────────
        case "session.status": {
          const sid = props.sessionID as string | undefined;
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
          break;
        // ── File edited (diff review) ──────────────────────
        case "file.edited": {
          // Increment counter to signal the diff review panel to refresh
          setFileEditCount((prev) => prev + 1);
          break;
        }
      }
    }

    return () => {
      appSSE.close();
      sessionSSE.close();
      if (flushTimerRef.current) {
        clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
      if (flushSubagentTimerRef.current) {
        clearTimeout(flushSubagentTimerRef.current);
        flushSubagentTimerRef.current = null;
      }
    };
  }, [refreshState, flushMessages, flushSubagentMessages]);

  /** Add an optimistic user message to the timeline.
   *  Uses a special `__optimistic__` prefix so it's identifiable.
   *  It's automatically replaced when the real server message arrives via SSE. */
  const addOptimisticMessage = useCallback(
    (text: string) => {
      const id = `__optimistic__${Date.now()}`;
      const msg: Message = {
        info: {
          role: "user",
          messageID: id,
          id,
          sessionID: activeSessionRef.current ?? undefined,
          time: Date.now() / 1000,
        },
        parts: [{ type: "text", text }],
      };
      messageMapRef.current.set(id, msg);
      flushMessages();
    },
    [flushMessages],
  );

  const clearMcpEditorOpen = useCallback(() => {
    setMcpEditorOpenPath(null);
    setMcpEditorOpenLine(null);
  }, []);

  const clearMcpTerminalFocus = useCallback(() => {
    setMcpTerminalFocusId(null);
  }, []);

  return {
    appState,
    messages,
    stats,
    busySessions,
    permissions,
    questions,
    sessionStatus,
    isLoadingMessages,
    watcherStatus,
    subagentMessages,
    fileEditCount,
    mcpEditorOpenPath,
    mcpEditorOpenLine,
    mcpTerminalFocusId,
    mcpAgentActivity,
    presenceClients,
    liveActivityEvents,
    refreshState,
    refreshMessages,
    clearPermission,
    clearQuestion,
    clearMcpEditorOpen,
    clearMcpTerminalFocus,
    addOptimisticMessage,
  };
}
