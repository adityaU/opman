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

/** DELETE helper */
async function apiDelete(path: string): Promise<void> {
  const res = await fetch(`/api${path}`, {
    method: "DELETE",
    headers: { ...authHeaders() },
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
}

/** PATCH helper */
async function apiPatch<T = void>(
  path: string,
  body?: unknown
): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "PATCH",
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

/** Add a new project by directory path. Returns the new project index and name. */
export async function addProject(
  path: string,
  name?: string
): Promise<{ index: number; name: string }> {
  return apiPost("/project/add", { path, name });
}

/** Remove a project by index */
export async function removeProject(index: number): Promise<void> {
  return apiPost("/project/remove", { index });
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

export interface NewSessionResponse {
  session_id: string;
}

export async function newSession(
  projectIdx: number
): Promise<NewSessionResponse> {
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

/** An image attachment to include with a message */
export interface ImageAttachment {
  /** Base64-encoded image data (no data: prefix) */
  base64: string;
  /** MIME type, e.g. "image/png" */
  mimeType: string;
  /** Original filename (for display) */
  name: string;
}

/** Send a message to a session, optionally overriding the model and including images */
export async function sendMessage(
  sessionId: string,
  text: string,
  model?: ModelRef,
  images?: ImageAttachment[]
): Promise<unknown> {
  const parts: Record<string, unknown>[] = [{ type: "text", text }];
  if (images && images.length > 0) {
    for (const img of images) {
      parts.push({ type: "image", image: img.base64, mimeType: img.mimeType });
    }
  }
  const body: Record<string, unknown> = { parts };
  if (model) {
    body.model = model;
  }
  return apiPost(`/session/${sessionId}/message`, body);
}

/** Abort a running session */
export async function abortSession(sessionId: string): Promise<void> {
  return apiPost(`/session/${sessionId}/abort`);
}

/** Delete a session */
export async function deleteSession(sessionId: string): Promise<void> {
  return apiDelete(`/session/${sessionId}`);
}

/** Rename a session (update its title) */
export async function renameSession(
  sessionId: string,
  title: string
): Promise<void> {
  return apiPatch(`/session/${sessionId}`, { title });
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

// ── Agents ─────────────────────────────────────────────

export interface AgentInfo {
  id: string;
  label: string;
  description: string;
}

/** Fetch available agents from the project's opencode config */
export async function fetchAgents(): Promise<AgentInfo[]> {
  try {
    return await apiFetch<AgentInfo[]>("/agents");
  } catch {
    // Fallback to defaults if endpoint not available
    return [
      { id: "coder", label: "Coder", description: "Default coding agent" },
      { id: "task", label: "Task", description: "Autonomous task agent" },
    ];
  }
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

/** Parse an opencode SSE event from the "opencode" event type.
 *  The opencode server wraps events in a `{directory, payload}` envelope,
 *  where `payload` contains `{type, properties}`. We unwrap it here. */
export function parseOpenCodeEvent(data: string): OpenCodeEvent | null {
  try {
    const raw = JSON.parse(data);
    // Unwrap the envelope if present
    if (raw && raw.payload && typeof raw.payload.type === "string") {
      return raw.payload as OpenCodeEvent;
    }
    // Fall back to top-level format (in case the proxy already unwrapped)
    if (raw && typeof raw.type === "string") {
      return raw as OpenCodeEvent;
    }
    return null;
  } catch {
    return null;
  }
}

// ═══════════════════════════════════════════════════════
// Git API
// ═══════════════════════════════════════════════════════

export interface GitFileEntry {
  path: string;
  status: string;
}

export interface GitStatusResponse {
  branch: string;
  staged: GitFileEntry[];
  unstaged: GitFileEntry[];
  untracked: GitFileEntry[];
}

export interface GitDiffResponse {
  diff: string;
}

export interface GitLogEntry {
  hash: string;
  short_hash: string;
  author: string;
  date: string;
  message: string;
}

export interface GitLogResponse {
  commits: GitLogEntry[];
}

export interface GitCommitResponse {
  hash: string;
  message: string;
}

/** Fetch structured git status for the active project */
export async function fetchGitStatus(): Promise<GitStatusResponse> {
  return apiFetch<GitStatusResponse>("/git/status");
}

/** Fetch diff for a file or all files */
export async function fetchGitDiff(
  file?: string,
  staged?: boolean
): Promise<GitDiffResponse> {
  const params = new URLSearchParams();
  if (file) params.set("file", file);
  if (staged) params.set("staged", "true");
  const qs = params.toString();
  return apiFetch<GitDiffResponse>(`/git/diff${qs ? `?${qs}` : ""}`);
}

/** Fetch recent commit log */
export async function fetchGitLog(
  limit?: number
): Promise<GitLogResponse> {
  const qs = limit ? `?limit=${limit}` : "";
  return apiFetch<GitLogResponse>(`/git/log${qs}`);
}

/** Stage files (empty array = stage all) */
export async function gitStage(files: string[] = []): Promise<void> {
  return apiPost("/git/stage", { files });
}

/** Unstage files (empty array = unstage all) */
export async function gitUnstage(files: string[] = []): Promise<void> {
  return apiPost("/git/unstage", { files });
}

/** Create a git commit */
export async function gitCommit(
  message: string
): Promise<GitCommitResponse> {
  return apiPost<GitCommitResponse>("/git/commit", { message });
}

/** Discard unstaged changes for files */
export async function gitDiscard(files: string[]): Promise<void> {
  return apiPost("/git/discard", { files });
}

// ── Git show (commit detail) ──────────────────────────

export interface GitShowFile {
  path: string;
  status: string;
}

export interface GitShowResponse {
  hash: string;
  author: string;
  date: string;
  message: string;
  diff: string;
  files: GitShowFile[];
}

/** Fetch full details (metadata + diff + files) for a single commit */
export async function fetchGitShow(hash: string): Promise<GitShowResponse> {
  return apiFetch<GitShowResponse>(
    `/git/show?hash=${encodeURIComponent(hash)}`
  );
}

// ── Git branches ──────────────────────────────────────

export interface GitBranchesResponse {
  current: string;
  local: string[];
  remote: string[];
}

export interface GitCheckoutResponse {
  branch: string;
  success: boolean;
  message?: string;
}

/** List all local and remote branches */
export async function fetchGitBranches(): Promise<GitBranchesResponse> {
  return apiFetch<GitBranchesResponse>("/git/branches");
}

/** Switch to a different branch */
export async function gitCheckout(branch: string): Promise<GitCheckoutResponse> {
  return apiPost("/git/checkout", { branch });
}

// ── Git AI Context helpers ────────────────────────────────

export interface GitRangeDiffResponse {
  branch: string;
  base: string;
  commits: GitLogEntry[];
  diff: string;
  files_changed: number;
}

export interface GitContextSummaryResponse {
  branch: string;
  recent_commits: GitLogEntry[];
  staged_count: number;
  unstaged_count: number;
  untracked_count: number;
  summary: string;
}

/** Fetch commit range diff between current branch and a base branch */
export async function fetchGitRangeDiff(
  base?: string,
  limit?: number
): Promise<GitRangeDiffResponse> {
  const params = new URLSearchParams();
  if (base) params.set("base", base);
  if (limit != null) params.set("limit", String(limit));
  const qs = params.toString();
  return apiFetch<GitRangeDiffResponse>(`/git/range-diff${qs ? `?${qs}` : ""}`);
}

/** Fetch a human-readable git context summary (branch, recent commits, working tree counts) */
export async function fetchGitContextSummary(): Promise<GitContextSummaryResponse> {
  return apiFetch<GitContextSummaryResponse>("/git/context-summary");
}

// ═══════════════════════════════════════════════════════
// Multi-session dashboard API
// ═══════════════════════════════════════════════════════

export interface SessionOverviewEntry {
  id: string;
  title: string;
  parentID: string;
  project_name: string;
  project_index: number;
  directory: string;
  is_busy: boolean;
  time: { created: number; updated: number };
  stats?: SessionStats;
}

export interface SessionsOverviewResponse {
  sessions: SessionOverviewEntry[];
  total: number;
  busy_count: number;
}

export interface SessionTreeNode {
  id: string;
  title: string;
  project_name: string;
  project_index: number;
  is_busy: boolean;
  stats?: SessionStats;
  children: SessionTreeNode[];
}

export interface SessionsTreeResponse {
  roots: SessionTreeNode[];
  total: number;
}

/** Fetch flat overview of all sessions across all projects */
export async function fetchSessionsOverview(): Promise<SessionsOverviewResponse> {
  return apiFetch<SessionsOverviewResponse>("/sessions/overview");
}

/** Fetch parent/child tree of all sessions */
export async function fetchSessionsTree(): Promise<SessionsTreeResponse> {
  return apiFetch<SessionsTreeResponse>("/sessions/tree");
}

// ═══════════════════════════════════════════════════════
// File browsing / editing API
// ═══════════════════════════════════════════════════════

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface FileBrowseResponse {
  path: string;
  entries: FileEntry[];
}

export interface FileReadResponse {
  path: string;
  content: string;
  language: string;
}

/** List directory contents */
export async function browseFiles(
  path?: string
): Promise<FileBrowseResponse> {
  const qs = path ? `?path=${encodeURIComponent(path)}` : "";
  return apiFetch<FileBrowseResponse>(`/files${qs}`);
}

/** Read file content */
export async function readFile(path: string): Promise<FileReadResponse> {
  return apiFetch<FileReadResponse>(
    `/file/read?path=${encodeURIComponent(path)}`
  );
}

/** Write file content */
export async function writeFile(
  path: string,
  content: string
): Promise<void> {
  return apiPost("/file/write", { path, content });
}

/** Get authenticated URL for raw binary file (images, audio, video, etc) */
export function rawFileUrl(path: string): string {
  const token = getToken();
  const qs = `path=${encodeURIComponent(path)}${token ? `&token=${encodeURIComponent(token)}` : ""}`;
  return `/api/file/raw?${qs}`;
}

/** Classify a file path into a render type for the editor */
export type FileRenderType =
  | "code"
  | "image"
  | "audio"
  | "video"
  | "markdown"
  | "html"
  | "mermaid"
  | "svg"
  | "csv"
  | "pdf"
  | "binary";

export function classifyFile(path: string): FileRenderType {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  // Images
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "avif"].includes(ext))
    return "image";
  // Audio
  if (["mp3", "wav", "ogg", "flac", "aac", "m4a", "weba"].includes(ext))
    return "audio";
  // Video
  if (["mp4", "webm", "ogv", "mov", "avi", "mkv"].includes(ext))
    return "video";
  // PDF
  if (ext === "pdf") return "pdf";
  // CSV
  if (ext === "csv") return "csv";
  // Markdown (special rendering)
  if (["md", "mdx", "markdown"].includes(ext)) return "markdown";
  // HTML / rendered markup
  if (["html", "htm"].includes(ext)) return "html";
  if (["mmd", "mermaid"].includes(ext)) return "mermaid";
  if (ext === "svg") return "svg";
  // Binary / office docs (we can't render these)
  if (["xlsx", "xls", "pptx", "ppt", "docx", "doc", "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib", "wasm", "bin"].includes(ext))
    return "binary";
  // Everything else: code
  return "code";
}

export interface EditorLspDiagnostic {
  file: string;
  lnum: number;
  col: number;
  severity: string;
  message: string;
  source: string;
}

export interface EditorDefinitionLocation {
  file: string;
  lnum: number;
  col: number;
}

export async function fetchEditorDiagnostics(
  path: string,
  sessionId: string
): Promise<{ diagnostics: EditorLspDiagnostic[]; available: boolean }> {
  return apiFetch(`/editor/lsp/diagnostics?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}`);
}

export async function fetchEditorHover(
  path: string,
  sessionId: string,
  line: number,
  col: number
): Promise<{ hover: string | null; available: boolean }> {
  return apiFetch(`/editor/lsp/hover?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}&line=${line}&col=${col}`);
}

export async function fetchEditorDefinition(
  path: string,
  sessionId: string,
  line: number,
  col: number
): Promise<{ locations: EditorDefinitionLocation[]; available: boolean }> {
  return apiFetch(`/editor/lsp/definition?path=${encodeURIComponent(path)}&session_id=${encodeURIComponent(sessionId)}&line=${line}&col=${col}`);
}

export async function formatEditorFile(
  path: string,
  sessionId: string
): Promise<{ formatted: boolean; content: string; available: boolean }> {
  return apiPost("/editor/lsp/format", { path, session_id: sessionId });
}

// ── Context Window ─────────────────────────────────────────────────

export interface ContextItem {
  label: string;
  tokens: number;
}

export interface ContextCategory {
  name: string;
  label: string;
  tokens: number;
  pct: number;
  color: string;
  items: ContextItem[];
}

export interface ContextWindowResponse {
  context_limit: number;
  total_used: number;
  usage_pct: number;
  categories: ContextCategory[];
  estimated_messages_remaining: number | null;
}

/** Fetch context window usage breakdown for a session */
export async function fetchContextWindow(
  sessionId?: string
): Promise<ContextWindowResponse | null> {
  try {
    const qs = sessionId
      ? `?session_id=${encodeURIComponent(sessionId)}`
      : "";
    return await apiFetch<ContextWindowResponse>(`/context-window${qs}`);
  } catch {
    return null;
  }
}

// ── File Edits (Diff Review) ────────────────────────────────────────

export interface FileEditEntry {
  path: string;
  original_content: string;
  new_content: string;
  timestamp: string;
  index: number;
}

export interface FileEditsResponse {
  session_id: string;
  edits: FileEditEntry[];
  file_count: number;
}

/** Fetch file edits for a session (for diff review panel) */
export async function fetchFileEdits(
  sessionId: string
): Promise<FileEditsResponse> {
  return apiFetch<FileEditsResponse>(
    `/session/${encodeURIComponent(sessionId)}/file-edits`
  );
}

// ── Cross-Session Search ────────────────────────────────────────────

export interface SearchResultEntry {
  session_id: string;
  session_title: string;
  project_name: string;
  message_id: string;
  role: string;
  snippet: string;
  timestamp: number;
}

export interface SearchResponse {
  query: string;
  results: SearchResultEntry[];
  total: number;
}

/** Search messages across all sessions in a project */
export async function searchMessages(
  projectIdx: number,
  query: string,
  limit?: number
): Promise<SearchResponse> {
  const params = new URLSearchParams({ q: query });
  if (limit) params.set("limit", String(limit));
  return apiFetch<SearchResponse>(
    `/project/${projectIdx}/search?${params.toString()}`
  );
}

// ── Watcher types ───────────────────────────────────────────────────

export interface WatcherConfigRequest {
  session_id: string;
  project_idx: number;
  idle_timeout_secs: number;
  continuation_message: string;
  include_original?: boolean;
  original_message?: string | null;
  hang_message?: string;
  hang_timeout_secs?: number;
}

export interface WatcherConfigResponse {
  session_id: string;
  project_idx: number;
  idle_timeout_secs: number;
  continuation_message: string;
  include_original: boolean;
  original_message: string | null;
  hang_message: string;
  hang_timeout_secs: number;
  /** "idle_countdown" | "running" | "waiting" | "inactive" */
  status: string;
  idle_since_secs: number | null;
}

export interface WatcherListEntry {
  session_id: string;
  session_title: string;
  project_name: string;
  idle_timeout_secs: number;
  status: string;
  idle_since_secs: number | null;
}

export interface WatcherSessionEntry {
  session_id: string;
  title: string;
  project_name: string;
  project_idx: number;
  is_current: boolean;
  is_active: boolean;
  has_watcher: boolean;
}

export interface WatcherStatusEvent {
  session_id: string;
  /** "created" | "deleted" | "triggered" | "countdown" | "cancelled" */
  action: string;
  idle_since_secs: number | null;
}

export interface WatcherMessageEntry {
  role: string;
  text: string;
}

// ── Watcher API ─────────────────────────────────────────────────────

/** List all active watchers */
export async function listWatchers(): Promise<WatcherListEntry[]> {
  return apiFetch<WatcherListEntry[]>("/watchers");
}

/** Create or update a session watcher */
export async function createWatcher(
  req: WatcherConfigRequest
): Promise<WatcherConfigResponse> {
  return apiPost<WatcherConfigResponse>("/watcher", req);
}

/** Delete a watcher for a session */
export async function deleteWatcher(sessionId: string): Promise<void> {
  return apiDelete(`/watcher/${encodeURIComponent(sessionId)}`);
}

/** Get watcher config and status for a single session */
export async function getWatcher(
  sessionId: string
): Promise<WatcherConfigResponse> {
  return apiFetch<WatcherConfigResponse>(
    `/watcher/${encodeURIComponent(sessionId)}`
  );
}

/** List all sessions for the watcher session picker */
export async function getWatcherSessions(): Promise<WatcherSessionEntry[]> {
  return apiFetch<WatcherSessionEntry[]>("/watcher/sessions");
}

/** Fetch user messages from a session for original-message picker */
export async function getWatcherMessages(
  sessionId: string
): Promise<WatcherMessageEntry[]> {
  return apiFetch<WatcherMessageEntry[]>(
    `/watcher/${encodeURIComponent(sessionId)}/messages`
  );
}

// ── Session Continuity types ────────────────────────────────────────

/** A single activity event in a session (fine-grained, real-time). */
export interface ActivityEvent {
  session_id: string;
  /** "file_edit" | "tool_call" | "terminal" | "permission" | "question" | "status" */
  kind: string;
  summary: string;
  detail?: string;
  timestamp: string;
}

/** Response for GET /api/activity. */
export interface ActivityFeedResponse {
  session_id: string;
  events: ActivityEvent[];
}

/** A connected client's presence info. */
export interface ClientPresence {
  client_id: string;
  /** "web" | "tui" */
  interface_type: string;
  focused_session?: string;
  last_seen: string;
}

/** Response for GET /api/presence. */
export interface PresenceResponse {
  clients: ClientPresence[];
}

/** Presence snapshot broadcast via SSE. */
export interface PresenceSnapshot {
  clients: ClientPresence[];
}

// ─── Missions ───────────────────────────────────────────

export type MissionStatus = "planned" | "active" | "blocked" | "completed";

export interface Mission {
  id: string;
  title: string;
  goal: string;
  next_action: string;
  status: MissionStatus;
  project_index: number;
  session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface MissionsListResponse {
  missions: Mission[];
}

export interface CreateMissionRequest {
  title: string;
  goal: string;
  next_action: string;
  status?: MissionStatus;
  project_index: number;
  session_id?: string | null;
}

export interface UpdateMissionRequest {
  title?: string;
  goal?: string;
  next_action?: string;
  status?: MissionStatus;
  project_index?: number;
  session_id?: string | null;
}

// ── Session Continuity API ──────────────────────────────────────────

/** Fetch recent activity events for a session. */
export async function fetchActivityFeed(
  sessionId: string
): Promise<ActivityFeedResponse> {
  return apiFetch<ActivityFeedResponse>(
    `/activity?session_id=${encodeURIComponent(sessionId)}`
  );
}

/** Get current connected clients. */
export async function fetchPresence(): Promise<PresenceResponse> {
  return apiFetch<PresenceResponse>("/presence");
}

/** Register or update this client's presence. */
export async function registerPresence(
  clientId: string,
  interfaceType: string,
  focusedSession?: string
): Promise<void> {
  await apiPost("/presence", {
    client_id: clientId,
    interface_type: interfaceType,
    focused_session: focusedSession ?? null,
  });
}

/** Deregister this client's presence (on tab close). */
export async function deregisterPresence(clientId: string): Promise<void> {
  await apiDelete("/presence");
  // Note: body not supported by DELETE in our helper, but the backend
  // can extract from query or we can send as POST. For simplicity,
  // we'll use a POST to a different endpoint or accept it via query.
}

export async function fetchMissions(): Promise<MissionsListResponse> {
  return apiFetch<MissionsListResponse>("/missions");
}

export async function createMission(req: CreateMissionRequest): Promise<Mission> {
  return apiPost<Mission>("/missions", req);
}

export async function updateMission(
  missionId: string,
  req: UpdateMissionRequest
): Promise<Mission> {
  return apiPatch<Mission>(`/missions/${encodeURIComponent(missionId)}`, req);
}

export async function deleteMission(missionId: string): Promise<void> {
  return apiDelete(`/missions/${encodeURIComponent(missionId)}`);
}

// ─── Personal Memory ─────────────────────────────────────

export type MemoryScope = "global" | "project" | "session";

export interface PersonalMemoryItem {
  id: string;
  label: string;
  content: string;
  scope: MemoryScope;
  project_index: number | null;
  session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface PersonalMemoryListResponse {
  memory: PersonalMemoryItem[];
}

export interface CreatePersonalMemoryRequest {
  label: string;
  content: string;
  scope: MemoryScope;
  project_index?: number | null;
  session_id?: string | null;
}

export interface UpdatePersonalMemoryRequest {
  label?: string;
  content?: string;
  scope?: MemoryScope;
  project_index?: number | null;
  session_id?: string | null;
}

export async function fetchPersonalMemory(): Promise<PersonalMemoryListResponse> {
  return apiFetch<PersonalMemoryListResponse>("/memory");
}

export async function createPersonalMemory(
  req: CreatePersonalMemoryRequest
): Promise<PersonalMemoryItem> {
  return apiPost<PersonalMemoryItem>("/memory", req);
}

export async function updatePersonalMemory(
  memoryId: string,
  req: UpdatePersonalMemoryRequest
): Promise<PersonalMemoryItem> {
  return apiPatch<PersonalMemoryItem>(`/memory/${encodeURIComponent(memoryId)}`, req);
}

export async function deletePersonalMemory(memoryId: string): Promise<void> {
  return apiDelete(`/memory/${encodeURIComponent(memoryId)}`);
}

// ─── Autonomy Controls ───────────────────────────────────

export type AutonomyMode = "observe" | "nudge" | "continue" | "autonomous";

export interface AutonomySettings {
  mode: AutonomyMode;
  updated_at: string;
}

export async function fetchAutonomySettings(): Promise<AutonomySettings> {
  return apiFetch<AutonomySettings>("/autonomy");
}

export async function updateAutonomySettings(
  mode: AutonomyMode
): Promise<AutonomySettings> {
  return apiPost<AutonomySettings>("/autonomy", { mode });
}

// ─── Routines ────────────────────────────────────────────

export type RoutineTrigger = "manual" | "on_session_idle" | "daily_summary";
export type RoutineAction = "review_mission" | "open_inbox" | "open_activity_feed";

export interface RoutineDefinition {
  id: string;
  name: string;
  trigger: RoutineTrigger;
  action: RoutineAction;
  mission_id: string | null;
  session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface RoutineRunRecord {
  id: string;
  routine_id: string;
  status: string;
  summary: string;
  created_at: string;
}

export interface RoutinesListResponse {
  routines: RoutineDefinition[];
  runs: RoutineRunRecord[];
}

export async function fetchRoutines(): Promise<RoutinesListResponse> {
  return apiFetch<RoutinesListResponse>("/routines");
}

export async function createRoutine(req: {
  name: string;
  trigger: RoutineTrigger;
  action: RoutineAction;
  mission_id?: string | null;
  session_id?: string | null;
}): Promise<RoutineDefinition> {
  return apiPost<RoutineDefinition>("/routines", req);
}

export async function deleteRoutine(routineId: string): Promise<void> {
  return apiDelete(`/routines/${encodeURIComponent(routineId)}`);
}

export async function updateRoutine(
  routineId: string,
  req: {
    name?: string;
    trigger?: RoutineTrigger;
    action?: RoutineAction;
    mission_id?: string | null;
    session_id?: string | null;
  }
): Promise<RoutineDefinition> {
  return apiPatch<RoutineDefinition>(`/routines/${encodeURIComponent(routineId)}`, req);
}

export async function runRoutine(
  routineId: string,
  req?: { summary?: string }
): Promise<RoutineRunRecord> {
  return apiPost<RoutineRunRecord>(`/routines/${encodeURIComponent(routineId)}/run`, req ?? {});
}

// ─── Delegation Board ────────────────────────────────────

export type DelegationStatus = "planned" | "running" | "completed";

export interface DelegatedWorkItem {
  id: string;
  title: string;
  assignee: string;
  scope: string;
  status: DelegationStatus;
  mission_id: string | null;
  session_id: string | null;
  subagent_session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface DelegatedWorkListResponse {
  items: DelegatedWorkItem[];
}

export async function fetchDelegatedWork(): Promise<DelegatedWorkListResponse> {
  return apiFetch<DelegatedWorkListResponse>("/delegation");
}

export async function createDelegatedWork(req: {
  title: string;
  assignee: string;
  scope: string;
  mission_id?: string | null;
  session_id?: string | null;
  subagent_session_id?: string | null;
}): Promise<DelegatedWorkItem> {
  return apiPost<DelegatedWorkItem>("/delegation", req);
}

export async function deleteDelegatedWork(itemId: string): Promise<void> {
  return apiDelete(`/delegation/${encodeURIComponent(itemId)}`);
}

export async function updateDelegatedWork(
  itemId: string,
  req: { status?: DelegationStatus }
): Promise<DelegatedWorkItem> {
  return apiPatch<DelegatedWorkItem>(`/delegation/${encodeURIComponent(itemId)}`, req);
}

// ─── Workspace Snapshots ───────────────────────────────────────────

/** Panel visibility flags within a workspace snapshot. */
export interface WorkspacePanels {
  sidebar: boolean;
  terminal: boolean;
  editor: boolean;
  git: boolean;
}

/** Panel layout sizes within a workspace snapshot. */
export interface WorkspaceLayout {
  sidebar_width: number;
  terminal_height: number;
  side_panel_width: number;
}

/** Terminal tab descriptor within a workspace snapshot. */
export interface WorkspaceTerminalTab {
  label: string;
  kind: string;
}

/** Saved workspace snapshot. */
export interface WorkspaceSnapshot {
  name: string;
  created_at: string;
  panels: WorkspacePanels;
  layout: WorkspaceLayout;
  open_files: string[];
  active_file: string | null;
  terminal_tabs: WorkspaceTerminalTab[];
  session_id: string | null;
  git_branch: string | null;
  is_template: boolean;
  recipe_description?: string | null;
  recipe_next_action?: string | null;
  is_recipe?: boolean;
}

/** Response listing all saved workspaces. */
export interface WorkspacesListResponse {
  workspaces: WorkspaceSnapshot[];
}

/** Fetch all saved workspace snapshots. */
export async function fetchWorkspaces(): Promise<WorkspacesListResponse> {
  return apiFetch<WorkspacesListResponse>("/workspaces");
}

/** Save (upsert) a workspace snapshot. */
export async function saveWorkspace(
  snapshot: WorkspaceSnapshot
): Promise<void> {
  await apiPost("/workspaces", { snapshot });
}

/** Delete a workspace snapshot by name. */
export async function deleteWorkspace(name: string): Promise<void> {
  await apiDelete(`/workspaces?name=${encodeURIComponent(name)}`);
}
