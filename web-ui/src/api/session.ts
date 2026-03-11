import { apiFetch, apiPost, apiDelete, apiPatch, apiPut } from "./client";
import type { Message, Provider, SlashCommand, TodoItem } from "../types";

// ── Message pagination ────────────────────────────────

/** Pagination options for fetchSessionMessages. */
export interface MessagePageOptions {
  /** Max number of messages to return. Omit or 0 for all. */
  limit?: number;
  /** Only return messages created before this Unix-ms timestamp (exclusive). */
  before?: number;
}

/** Response shape from the paginated messages endpoint. */
export interface MessagePageResponse {
  messages: Message[];
  /** True if there are older messages available before this page. */
  has_more: boolean;
  /** Total number of messages in the session (before pagination). */
  total: number;
}

export async function fetchSessionMessages(
  sessionId: string,
  page?: MessagePageOptions
): Promise<MessagePageResponse> {
  const params = new URLSearchParams();
  if (page?.limit && page.limit > 0) params.set("limit", String(page.limit));
  if (page?.before) params.set("before", String(page.before));
  const qs = params.toString();
  const url = `/session/${sessionId}/messages${qs ? `?${qs}` : ""}`;
  const data = await apiFetch<unknown>(url);
  if (data && typeof data === "object" && !Array.isArray(data)) {
    const resp = data as Record<string, unknown>;
    if ("messages" in resp && Array.isArray(resp.messages)) {
      return {
        messages: resp.messages as Message[],
        has_more: resp.has_more === true,
        total: typeof resp.total === "number" ? resp.total : resp.messages.length,
      };
    }
    const msgs = Object.values(resp) as Message[];
    return { messages: msgs, has_more: false, total: msgs.length };
  }
  if (Array.isArray(data)) {
    return { messages: data as Message[], has_more: false, total: data.length };
  }
  return { messages: [], has_more: false, total: 0 };
}

// ── Message sending & lifecycle ───────────────────────

/** Model reference for the message endpoint */
export interface ModelRef {
  providerID: string;
  modelID: string;
}

/** An image attachment to include with a message */
export interface ImageAttachment {
  /** Base64-encoded image data (no data: prefix) */
  base64: string;
  /** MIME type, e.g. "image/png" */
  mimeType: string;
  /** Original filename (for display) */
  name: string;
}

export async function sendMessage(
  sessionId: string,
  text: string,
  model?: ModelRef,
  images?: ImageAttachment[],
  agent?: string
): Promise<unknown> {
  const parts: Record<string, unknown>[] = [{ type: "text", text }];
  if (images && images.length > 0) {
    for (const img of images) {
      parts.push({ type: "image", image: img.base64, mimeType: img.mimeType });
    }
  }
  const body: Record<string, unknown> = { parts };
  if (model) body.model = model;
  if (agent) body.agent = agent;
  return apiPost(`/session/${sessionId}/message`, body);
}

export async function abortSession(sessionId: string): Promise<void> {
  return apiPost(`/session/${sessionId}/abort`);
}

export async function deleteSession(sessionId: string): Promise<void> {
  return apiDelete(`/session/${sessionId}`);
}

export async function renameSession(sessionId: string, title: string): Promise<void> {
  return apiPatch(`/session/${sessionId}`, { title });
}

// ── Commands ──────────────────────────────────────────

export async function executeCommand(
  sessionId: string,
  command: string,
  args?: string,
  model?: string
): Promise<unknown> {
  return apiPost(`/session/${sessionId}/command`, {
    command,
    arguments: args || "",
    ...(model ? { model } : {}),
  });
}

export async function fetchCommands(): Promise<SlashCommand[]> {
  const data = await apiFetch<unknown>("/commands");
  if (Array.isArray(data)) return data as SlashCommand[];
  return [];
}

// ── Providers ─────────────────────────────────────────

interface ProvidersResponse {
  all: Provider[];
  connected: string[];
  default: Record<string, string>;
}

export async function fetchProviders(): Promise<ProvidersResponse> {
  const data = await apiFetch<unknown>("/providers");
  if (data && typeof data === "object" && !Array.isArray(data)) {
    const resp = data as Record<string, unknown>;
    return {
      all: (resp.all as Provider[]) || [],
      connected: (resp.connected as string[]) || [],
      default: (resp.default as Record<string, string>) || {},
    };
  }
  if (Array.isArray(data)) {
    return { all: data as Provider[], connected: [], default: {} };
  }
  return { all: [], connected: [], default: {} };
}

// ── Permissions & Questions ───────────────────────────

export async function replyPermission(
  requestId: string,
  reply: "once" | "always" | "reject"
): Promise<void> {
  return apiPost(`/permission/${requestId}/reply`, { reply });
}

export async function replyQuestion(
  requestId: string,
  answers: string[][]
): Promise<void> {
  return apiPost(`/question/${requestId}/reply`, { answers });
}

// ── Todos ─────────────────────────────────────────────

export async function fetchSessionTodos(sessionId: string): Promise<TodoItem[]> {
  return apiFetch<TodoItem[]>(`/session/${sessionId}/todos`);
}

/** Full-replace all todos for a session. Mirrors the TUI's save_todos_to_db semantics. */
export async function updateSessionTodos(
  sessionId: string,
  todos: Array<{ content: string; status: string; priority: string }>
): Promise<TodoItem[]> {
  return apiPut<TodoItem[]>(`/session/${sessionId}/todos`, todos);
}

// ── Agents ────────────────────────────────────────────

export interface AgentInfo {
  id: string;
  label: string;
  description: string;
  mode?: string;
  hidden?: boolean;
  native?: boolean;
  color?: string;
}

export async function fetchAgents(): Promise<AgentInfo[]> {
  try {
    return await apiFetch<AgentInfo[]>("/agents");
  } catch {
    return [
      { id: "build", label: "Build", description: "Default coding agent", mode: "primary", native: true },
      { id: "plan", label: "Plan", description: "Planning and design agent", mode: "all", native: true },
    ];
  }
}


