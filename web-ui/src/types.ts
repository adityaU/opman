// ── OpenCode message types ──────────────────────────────────────────

/** A single message part (text, tool call, tool result, etc.) */
export interface MessagePart {
  type: string;

  // Common fields present on all parts from SSE events
  id?: string;            // part ID (from SSE message.part.updated)
  sessionID?: string;     // session this part belongs to
  messageID?: string;     // message this part belongs to

  // Text part
  text?: string;

  // Tool part (type: "tool") – actual API structure
  tool?: string;          // tool name, e.g. "neovim_neovim_edit_and_save"
  callID?: string;        // tool call ID
  state?: ToolPartState;  // input/output/status

  // Step parts (type: "step-start" / "step-finish")
  stepID?: string;

  // Subtask part (type: "subtask")
  prompt?: string;
  description?: string;
  agent?: string;

  // Legacy / fallback fields
  toolCallId?: string;
  toolName?: string;
  args?: Record<string, unknown>;
  result?: string | unknown;

  // File reference
  filename?: string;
  url?: string;
  mime?: string;
  source?: string;
}

/** State of a tool part as returned by the API */
export interface ToolPartState {
  input?: Record<string, unknown> | string;
  output?: string;
  status?: "completed" | "pending" | "running" | "error";
  title?: string;
  time?: { start?: number; end?: number };
  metadata?: { truncated?: boolean };
  attachments?: unknown[];
}

/** Model reference – the API returns either a plain string or an object. */
export type ModelRef =
  | string
  | { modelID: string; providerID: string };

/** Message info metadata — matches opencode's Message (UserMessage | AssistantMessage) */
export interface MessageInfo {
  role: "user" | "assistant" | "system" | "tool";
  /** Message ID — called `id` in SSE events, `messageID` in REST responses */
  messageID?: string;
  id?: string;
  sessionID?: string;
  time?: number | { created?: number; completed?: number };
  model?: ModelRef;
  modelID?: string;
  providerID?: string;
  system?: boolean;
  agent?: string;
  cost?: number;
  tokens?: {
    input?: number;
    output?: number;
    reasoning?: number;
    total?: number;
    cache?: { read?: number; write?: number };
  };
  error?: unknown;
  finish?: string;
  parentID?: string;
  mode?: string;
  path?: { cwd?: string; root?: string };
  variant?: string;
}

/** A single message in a session */
export interface Message {
  info: MessageInfo;
  parts: MessagePart[];
  metadata?: {
    time?: { created?: number; completed?: number };
    tokens?: { input?: number; output?: number; reasoning?: number; cache_read?: number; cache_write?: number };
    cost?: number;
  };
}

// ── Permission & Question types ─────────────────────────────────────

export interface PermissionRequest {
  id: string;
  sessionID: string;
  toolName: string;
  description?: string;
  args?: Record<string, unknown>;
  time: number;
}

export interface QuestionRequest {
  id: string;
  sessionID: string;
  title: string;
  questions: QuestionItem[];
  time: number;
}

export interface QuestionItem {
  text: string;
  type: "text" | "select" | "confirm";
  options?: string[];
  multiple?: boolean;
}

// ── Provider & Model types ──────────────────────────────────────────

export interface Provider {
  id: string;
  name: string;
  models: Record<string, ModelInfo>;
}

export interface ModelInfo {
  name: string;
  id: string;
  limit?: { context?: number; output?: number };
  features?: string[];
}

// ── Command types ───────────────────────────────────────────────────

export interface SlashCommand {
  name: string;
  description?: string;
  args?: string;
}

// ── Todo types ──────────────────────────────────────────────────────

export interface TodoItem {
  id: string;
  sessionID: string;
  content: string;
  status: "pending" | "in_progress" | "completed" | "cancelled";
  priority?: string;
}

// ── SSE event types ─────────────────────────────────────────────────

export type OpenCodeEventType =
  | "message.updated"
  | "message.removed"
  | "message.part.updated"
  | "message.part.delta"
  | "message.part.removed"
  | "session.status"
  | "session.created"
  | "session.updated"
  | "session.deleted"
  | "permission.asked"
  | "question.asked"
  | "file.edited"
  | "todo.updated";

export interface OpenCodeEvent {
  type: OpenCodeEventType;
  properties: Record<string, unknown>;
}
