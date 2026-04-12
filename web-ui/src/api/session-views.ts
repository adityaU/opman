import { apiFetch } from "./client";
import type { OpenCodeEvent } from "../types";
import type { SessionStats } from "./state";

// ── Session events SSE ────────────────────────────────

export function createSessionEventsSSE(): EventSource {
  // Cookie auth: browser sends opman_token cookie automatically.
  return new EventSource(`/api/session/events`);
}

export function parseOpenCodeEvent(data: string): OpenCodeEvent | null {
  try {
    const raw = JSON.parse(data);
    if (raw && raw.payload && typeof raw.payload.type === "string") {
      return raw.payload as OpenCodeEvent;
    }
    if (raw && typeof raw.type === "string") {
      return raw as OpenCodeEvent;
    }
    return null;
  } catch {
    return null;
  }
}

// ── Multi-session dashboard ───────────────────────────

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

export async function fetchSessionsOverview(): Promise<SessionsOverviewResponse> {
  return apiFetch<SessionsOverviewResponse>("/sessions/overview");
}

export async function fetchSessionsTree(): Promise<SessionsTreeResponse> {
  return apiFetch<SessionsTreeResponse>("/sessions/tree");
}

// ── Context Window ────────────────────────────────────

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

export async function fetchContextWindow(
  sessionId?: string
): Promise<ContextWindowResponse | null> {
  try {
    const qs = sessionId ? `?session_id=${encodeURIComponent(sessionId)}` : "";
    return await apiFetch<ContextWindowResponse>(`/context-window${qs}`);
  } catch {
    return null;
  }
}

// ── File Edits (Diff Review) ──────────────────────────

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

export async function fetchFileEdits(sessionId: string): Promise<FileEditsResponse> {
  return apiFetch<FileEditsResponse>(
    `/session/${encodeURIComponent(sessionId)}/file-edits`
  );
}

// ── Cross-Session Search ──────────────────────────────

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
