import type {
  Message,
  Provider,
  SlashCommand,
  TodoItem,
  OpenCodeEvent,
} from "./types";

// ── Token management ──────────────────────────────────

/** Get the stored auth token */
export function getToken(): string | null {
  return sessionStorage.getItem("opman_token");
}

/** Store auth token */
export function setToken(token: string) {
  sessionStorage.setItem("opman_token", token);
}

/** Clear auth token */
export function clearToken() {
  sessionStorage.removeItem("opman_token");
}

/** Build auth headers */
function authHeaders(): Record<string, string> {
  const token = getToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

/** Typed GET fetch helper */
async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
      ...init?.headers,
    },
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status} ${res.statusText}`);
  return res.json();
}

/** POST helper */
async function apiPost<T = void>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  const text = await res.text();
  if (text) return JSON.parse(text) as T;
  return undefined as unknown as T;
}

// ── Types (from existing app state) ───────────────────

export interface SessionInfo {
  id: string;
  title: string;
  parentID: string;
  directory: string;
  time: { created: number; updated: number };
}

export interface ProjectInfo {
  name: string;
  path: string;
  index: number;
  active_session: string | null;
  sessions: SessionInfo[];
  git_branch: string;
  busy_sessions: string[];
}

export interface AppState {
  projects: ProjectInfo[];
  active_project: number;
  panels: PanelVisibility;
  focused: string;
}

export interface PanelVisibility {
  sidebar: boolean;
  terminal_pane: boolean;
  neovim_pane: boolean;
  integrated_terminal: boolean;
  git_panel: boolean;
}

export interface SessionStats {
  cost: number;
  input_tokens: number;
  output_tokens: number;
  reasoning_tokens: number;
  cache_read: number;
  cache_write: number;
}

/** Theme colors — 15 hex strings matching the TUI's ThemeColors struct. */
export interface ThemeColors {
  primary: string;
  secondary: string;
  accent: string;
  background: string;
  background_panel: string;
  background_element: string;
  text: string;
  text_muted: string;
  border: string;
  border_active: string;
  border_subtle: string;
  error: string;
  warning: string;
  success: string;
  info: string;
}

// ── Auth ───────────────────────────────────────────────

export async function login(
  username: string,
  password: string
): Promise<string> {
  const res = await fetch("/api/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password }),
  });
  if (!res.ok) throw new Error("Invalid credentials");
  const data = await res.json();
  return data.token;
}

export async function verifyToken(): Promise<boolean> {
  const token = getToken();
  if (!token) return false;
  try {
    const res = await fetch("/api/auth/verify", {
      headers: { Authorization: `Bearer ${token}` },
    });
    return res.ok;
  } catch {
    return false;
  }
}

// ── State ──────────────────────────────────────────────

export async function fetchAppState(): Promise<AppState> {
  return apiFetch<AppState>("/state");
}

export async function fetchSessionStats(
  sessionId: string
): Promise<SessionStats | null> {
  try {
    return await apiFetch<SessionStats>(`/session/${sessionId}/stats`);
  } catch {
    return null;
  }
}

export async function fetchTheme(): Promise<ThemeColors | null> {
  try {
    return await apiFetch<ThemeColors>("/theme");
  } catch {
    return null;
  }
}

// ── Actions (existing) ─────────────────────────────────

export async function switchProject(index: number): Promise<void> {
  return apiPost("/project/switch", { index });
}

export async function selectSession(
  projectIdx: number,
  sessionId: string
): Promise<void> {
  return apiPost("/session/select", {
    project_idx: projectIdx,
    session_id: sessionId,
  });
}

export async function newSession(projectIdx: number): Promise<void> {
  return apiPost("/session/new", { project_idx: projectIdx });
}

export async function togglePanel(panel: string): Promise<void> {
  return apiPost("/panel/toggle", { panel });
}

export async function focusPanel(panel: string): Promise<void> {
  return apiPost("/panel/focus", { panel });
}

// ── Web PTY management ────────────────────────────────

export interface SpawnPtyResponse {
  id: string;
  ok: boolean;
}

export async function spawnPty(
  kind: string,
  id: string,
  rows: number,
  cols: number,
  sessionId?: string
): Promise<SpawnPtyResponse> {
  const body: Record<string, unknown> = { kind, id, rows, cols };
  if (sessionId) body.session_id = sessionId;
  return apiPost<SpawnPtyResponse>("/pty/spawn", body);
}

export async function ptyWrite(id: string, data: string): Promise<void> {
  return apiPost("/pty/write", { id, data });
}

export async function ptyResize(
  id: string,
  rows: number,
  cols: number
): Promise<void> {
  return apiPost("/pty/resize", { id, rows, cols });
}

export async function ptyKill(id: string): Promise<void> {
  return apiPost("/pty/kill", { id });
}

export async function ptyList(): Promise<string[]> {
  return apiFetch<string[]>("/pty/list");
}

export function createPtySSE(id: string): EventSource {
  const token = getToken();
  return new EventSource(
    `/api/pty/stream?id=${encodeURIComponent(id)}&token=${encodeURIComponent(token || "")}`
  );
}

// ── App events SSE ─────────────────────────────────────

export function createEventsSSE(): EventSource {
  const token = getToken();
  return new EventSource(
    `/api/events?token=${encodeURIComponent(token || "")}`
  );
}

// ═══════════════════════════════════════════════════════
// NEW: Proxy endpoints (opencode server)
// ═══════════════════════════════════════════════════════

// ── Session messages ───────────────────────────────────

/** Fetch all messages for a session, sorted by creation time. */
export async function fetchSessionMessages(
  sessionId: string
): Promise<Message[]> {
  const data = await apiFetch<unknown>(`/session/${sessionId}/messages`);

  // Response format: { messages: [...] }
  if (data && typeof data === "object" && !Array.isArray(data)) {
    const resp = data as Record<string, unknown>;
    if ("messages" in resp && Array.isArray(resp.messages)) {
      return resp.messages as Message[];
    }
    // Legacy fallback: object keyed by message ID
    return Object.values(resp) as Message[];
  }
  // Legacy fallback: plain array
  if (Array.isArray(data)) {
    return data as Message[];
  }
  return [];
}

/** Model reference for the message endpoint */
export interface ModelRef {
  providerID: string;
  modelID: string;
}

/** Send a message to a session, optionally overriding the model */
export async function sendMessage(
  sessionId: string,
  text: string,
  model?: ModelRef
): Promise<unknown> {
  const body: Record<string, unknown> = {
    parts: [{ type: "text", text }],
  };
  if (model) {
    body.model = model;
  }
  return apiPost(`/session/${sessionId}/message`, body);
}

/** Abort a running session */
export async function abortSession(sessionId: string): Promise<void> {
  return apiPost(`/session/${sessionId}/abort`);
}

// ── Commands ───────────────────────────────────────────

/** Execute a slash command on a session */
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

/** List available slash commands */
export async function fetchCommands(): Promise<SlashCommand[]> {
  const data = await apiFetch<unknown>("/commands");
  if (Array.isArray(data)) return data as SlashCommand[];
  return [];
}

// ── Providers ──────────────────────────────────────────

/** Raw providers API response */
interface ProvidersResponse {
  all: Provider[];
  connected: string[];
  default: Record<string, string>;
}

/** Fetch available providers and models.
 *  Returns { all, connected, defaults } from the API. */
export async function fetchProviders(): Promise<ProvidersResponse> {
  const data = await apiFetch<unknown>("/providers");
  // API returns { all: Provider[], connected: string[], default: {} }
  if (data && typeof data === "object" && !Array.isArray(data)) {
    const resp = data as Record<string, unknown>;
    return {
      all: (resp.all as Provider[]) || [],
      connected: (resp.connected as string[]) || [],
      default: (resp.default as Record<string, string>) || {},
    };
  }
  // Fallback: if it's somehow a flat array
  if (Array.isArray(data)) {
    return { all: data as Provider[], connected: [], default: {} };
  }
  return { all: [], connected: [], default: {} };
}

// ── Permissions & Questions ────────────────────────────

/** Reply to a permission request */
export async function replyPermission(
  requestId: string,
  reply: "once" | "always" | "reject"
): Promise<void> {
  return apiPost(`/permission/${requestId}/reply`, { reply });
}

/** Reply to a question */
export async function replyQuestion(
  requestId: string,
  answers: string[][]
): Promise<void> {
  return apiPost(`/question/${requestId}/reply`, { answers });
}

// ── Todos ──────────────────────────────────────────────

/** Fetch todos for a session */
export async function fetchSessionTodos(
  sessionId: string
): Promise<TodoItem[]> {
  return apiFetch<TodoItem[]>(`/session/${sessionId}/todos`);
}

// ── Themes ─────────────────────────────────────────────

/** Theme preview with resolved colors */
export interface ThemePreview {
  name: string;
  colors: ThemeColors;
}

/** Fetch all available themes with preview colors */
export async function fetchThemes(): Promise<ThemePreview[]> {
  return apiFetch<ThemePreview[]>("/themes");
}

/** Switch the active theme by name. Returns the new theme colors. */
export async function switchTheme(name: string): Promise<ThemeColors> {
  return apiPost<ThemeColors>("/theme/switch", { name });
}

// ── Session events SSE (proxied from opencode) ─────────

/** Create an SSE connection for opencode events (message updates, permissions, etc.) */
export function createSessionEventsSSE(): EventSource {
  const token = getToken();
  return new EventSource(
    `/api/session/events?token=${encodeURIComponent(token || "")}`
  );
}

/** Parse an opencode SSE event from the "opencode" event type */
export function parseOpenCodeEvent(data: string): OpenCodeEvent | null {
  try {
    return JSON.parse(data) as OpenCodeEvent;
  } catch {
    return null;
  }
}
