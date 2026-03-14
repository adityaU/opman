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

export interface SSEState {
  appState: AppState | null;
  messages: Message[];
  stats: SessionStats | null;
  busySessions: Set<string>;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  sessionStatus: "idle" | "busy";
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
  addOptimisticMessage: (text: string) => void;
  /** Load older messages (pagination). Returns true if more messages exist. */
  loadOlderMessages: () => Promise<boolean>;
}
