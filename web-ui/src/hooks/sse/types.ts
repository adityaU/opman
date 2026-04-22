import type {
  AppState,
  SessionStats,
  ActivityEvent,
  ClientPresence,
} from "../../api";
import type { Message, PermissionRequest, QuestionRequest } from "../../types";

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

/** SSE connection health status. */
export type SSEConnectionStatus = "connected" | "reconnecting" | "disconnected";

/**
 * Per-session status — mirrors OpenCode's SessionStatus schema.
 * idle: not running.  busy: actively processing.  retry: hit a retryable
 * error, will retry after `next` (epoch ms).
 */
export type SessionStatus =
  | { type: "idle" }
  | { type: "busy" }
  | { type: "retry"; attempt: number; message: string; next: number };

export const SESSION_IDLE: SessionStatus = { type: "idle" };
export const SESSION_BUSY: SessionStatus = { type: "busy" };

export interface SSEState {
  appState: AppState | null;
  messages: Message[];
  stats: SessionStats | null;
  busySessions: Set<string>;
  /** Full per-session status map (idle entries are omitted — absent = idle). */
  sessionStatuses: Readonly<Record<string, SessionStatus>>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  sessionStatus: SessionStatus;
  /** Aggregate SSE connection health (worst-case of app + session streams). */
  connectionStatus: SSEConnectionStatus;
  /** True while loading messages for a newly-selected session */
  isLoadingMessages: boolean;
  /** True while loading older messages (pagination scroll-up) */
  isLoadingOlder: boolean;
  /** True if there are older messages available to load */
  hasOlderMessages: boolean;
  /** Total message count in the session (reported by server) */
  totalMessageCount: number;
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
  /** Permission requests from non-active sessions (e.g. subagent in another session). */
  crossSessionPermissions: PermissionRequest[];
  /** Question requests from non-active sessions (e.g. subagent in another session). */
  crossSessionQuestions: QuestionRequest[];
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
  addOptimisticMessage: (text: string, images?: { base64: string; mimeType: string; name: string }[]) => void;
  /** Remove all optimistic messages from the map (e.g. on send failure). */
  clearOptimistic: () => void;
  /** Load older messages (pagination). Returns true if more messages exist. */
  loadOlderMessages: () => Promise<boolean>;
  /** Signal that a user-initiated session switch is expected.
   *  Call this before selectSession/newSession/switchProject so the next
   *  appState update is allowed to change the active session. */
  expectSessionSwitch: () => void;
  /** Temporarily block background SSE-driven session adoption.
   *  Use for non-switch UI actions (like model selection) that should never
   *  cause the active session to change even if another session is busy. */
  blockSessionAdoption: (ms?: number) => void;
  /** Optimistically begin a session switch — clears messages and shows loading
   *  state immediately at click-time, before any async API calls.
   *  Restores from cache if available (no shimmer).
   *  Pass `projectIdx` when switching to a session in a different project. */
  beginSessionSwitch: (targetSid: string, projectIdx?: number) => void;
  /** Stable callback for checking if a session is busy — avoids passing the Set reference
   *  to children (which would defeat React.memo on every busy/idle SSE event). */
  isSessionBusy: (sid: string) => boolean;
}
