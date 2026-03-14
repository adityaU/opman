import { apiFetch, apiPost, apiDelete, clearToken } from "./client";

// ── Watcher types ─────────────────────────────────────

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

// ── Watcher API ───────────────────────────────────────

export async function listWatchers(): Promise<WatcherListEntry[]> {
  return apiFetch<WatcherListEntry[]>("/watchers");
}

export async function createWatcher(req: WatcherConfigRequest): Promise<WatcherConfigResponse> {
  return apiPost<WatcherConfigResponse>("/watcher", req);
}

export async function deleteWatcher(sessionId: string): Promise<void> {
  return apiDelete(`/watcher/${encodeURIComponent(sessionId)}`);
}

export async function getWatcher(sessionId: string): Promise<WatcherConfigResponse> {
  return apiFetch<WatcherConfigResponse>(
    `/watcher/${encodeURIComponent(sessionId)}`
  );
}

export async function getWatcherSessions(): Promise<WatcherSessionEntry[]> {
  return apiFetch<WatcherSessionEntry[]>("/watcher/sessions");
}

export async function getWatcherMessages(sessionId: string): Promise<WatcherMessageEntry[]> {
  return apiFetch<WatcherMessageEntry[]>(
    `/watcher/${encodeURIComponent(sessionId)}/messages`
  );
}

// ── Activity Feed types ───────────────────────────────

export interface ActivityEvent {
  session_id: string;
  /** "file_edit" | "tool_call" | "terminal" | "permission" | "question" | "status" */
  kind: string;
  summary: string;
  detail?: string;
  timestamp: string;
}

export interface ActivityFeedResponse {
  session_id: string;
  events: ActivityEvent[];
}

// ── Presence types ────────────────────────────────────

export interface ClientPresence {
  client_id: string;
  /** "web" | "tui" */
  interface_type: string;
  focused_session?: string;
  last_seen: string;
}

export interface PresenceResponse {
  clients: ClientPresence[];
}

export interface PresenceSnapshot {
  clients: ClientPresence[];
}

// ── Activity / Presence API ───────────────────────────

export async function fetchActivityFeed(sessionId: string): Promise<ActivityFeedResponse> {
  return apiFetch<ActivityFeedResponse>(
    `/activity?session_id=${encodeURIComponent(sessionId)}`
  );
}

export async function fetchPresence(): Promise<PresenceResponse> {
  return apiFetch<PresenceResponse>("/presence");
}

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

export async function deregisterPresence(clientId: string): Promise<void> {
  const res = await fetch("/api/presence", {
    method: "DELETE",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ client_id: clientId }),
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
}
