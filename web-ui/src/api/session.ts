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
  agent?: string,
  runner?: string,
  effort?: string,
  permission?: string,
): Promise<unknown> {
  const parts: Record<string, unknown>[] = [{ type: "text", text }];
  if (images && images.length > 0) {
    for (const img of images) {
      // OpenCode expects FilePartInput: { type: "file", mime, url (data-URL), filename? }
      parts.push({
        type: "file",
        mime: img.mimeType,
        url: `data:${img.mimeType};base64,${img.base64}`,
        filename: img.name,
      });
    }
  }
  const body: Record<string, unknown> = { parts };
  if (model) body.model = model;
  if (agent) body.agent = agent;
  if (runner) body.runner = runner;
  if (effort) body.effort = effort;
  if (permission) body.permission = permission;
  return apiPost(`/session/${sessionId}/message`, body);
}

export async function abortSession(sessionId: string): Promise<void> {
  return apiPost(`/session/${sessionId}/abort`);
}

// ── Queued follow-up prompts ──────────────────────────

/** Fetch the follow-up prompts queued while the agent is busy (oldest first). */
export async function fetchQueue(sessionId: string): Promise<string[]> {
  const r = await apiFetch<{ pending?: string[] }>(`/session/${sessionId}/queue`);
  return r.pending ?? [];
}

/** Remove one queued follow-up by index; returns the remaining queue. */
export async function removeQueuedMessage(sessionId: string, index: number): Promise<string[]> {
  const r = await apiFetch<{ pending?: string[] }>(
    `/session/${sessionId}/queue/${index}`,
    { method: "DELETE" }
  );
  return r.pending ?? [];
}

/** Drop every queued follow-up for a session. */
export async function clearQueue(sessionId: string): Promise<void> {
  await apiFetch<unknown>(`/session/${sessionId}/queue`, { method: "DELETE" });
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

export async function fetchProviders(runner?: string): Promise<ProvidersResponse> {
  const path = runner ? `/providers?runner=${encodeURIComponent(runner)}` : "/providers";
  const data = await apiFetch<unknown>(path);
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

export async function rejectQuestion(requestId: string): Promise<void> {
  return apiPost(`/question/${requestId}/reject`, {});
}

/** Pending permissions/questions tracked server-side (survives page reload). */
export interface PendingResponse {
  permissions: Record<string, unknown>[];
  questions: Record<string, unknown>[];
}

export async function fetchPending(): Promise<PendingResponse> {
  try {
    const data = await apiFetch<PendingResponse>("/pending");
    return {
      permissions: Array.isArray(data?.permissions) ? data.permissions : [],
      questions: Array.isArray(data?.questions) ? data.questions : [],
    };
  } catch {
    return { permissions: [], questions: [] };
  }
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
  runner?: string;
  id: string;
  label: string;
  description: string;
  mode?: string;
  hidden?: boolean;
  native?: boolean;
  color?: string;
}

export const RUNNER_AGENT_FALLBACKS: Record<string, AgentInfo[]> = {
  opencode: [
    { id: "build", label: "Build", description: "Default coding agent", mode: "primary", native: true },
    { id: "plan", label: "Plan", description: "Planning and design agent", mode: "all", native: true },
  ],
  "claude-code": [{ id: "default", label: "Default", description: "Claude Code default agent", mode: "primary", native: true }],
  claude: [{ id: "default", label: "Default", description: "Claude default agent", mode: "primary", native: true }],
  codex: [{ id: "default", label: "Default", description: "Codex default agent", mode: "primary", native: true }],
};

export function runnerFallbackAgents(runner = "opencode"): AgentInfo[] {
  return RUNNER_AGENT_FALLBACKS[runner] || RUNNER_AGENT_FALLBACKS.opencode;
}

export async function fetchAgents(runner = "opencode"): Promise<AgentInfo[]> {
  try {
    const agents = await apiFetch<AgentInfo[]>("/agents?runner=" + encodeURIComponent(runner));
    const runnerTagged = agents.filter((agent) => agent.runner);
    if (runnerTagged.length > 0) return runnerTagged.filter((agent) => agent.runner === runner);
    if (agents.length > 0) return agents;
    if (runner !== "opencode") return RUNNER_AGENT_FALLBACKS[runner] || RUNNER_AGENT_FALLBACKS.opencode;
    return agents;
  } catch {
    return RUNNER_AGENT_FALLBACKS[runner] || RUNNER_AGENT_FALLBACKS.opencode;
  }
}
