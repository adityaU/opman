import { apiFetch, apiPost, apiDelete, apiPatch } from "./client";

// ── Personal Memory types ─────────────────────────────

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

// ── Personal Memory API ───────────────────────────────

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

// ── Autonomy Controls ─────────────────────────────────

export type AutonomyMode = "observe" | "nudge" | "continue" | "autonomous";

export interface AutonomySettings {
  mode: AutonomyMode;
  updated_at: string;
}

export async function fetchAutonomySettings(): Promise<AutonomySettings> {
  return apiFetch<AutonomySettings>("/autonomy");
}

export async function updateAutonomySettings(mode: AutonomyMode): Promise<AutonomySettings> {
  return apiPost<AutonomySettings>("/autonomy", { mode });
}

// ── Active Memory ────────────────────────────────────────

export interface ActiveMemoryResponse {
  memory: Array<{
    id: string;
    label: string;
    content: string;
    scope: string;
    project_index?: number | null;
    session_id?: string | null;
    created_at: string;
    updated_at: string;
  }>;
}

export async function fetchActiveMemory(
  projectIndex?: number,
  sessionId?: string | null,
): Promise<ActiveMemoryResponse> {
  const params = new URLSearchParams();
  if (projectIndex !== undefined) params.set("project_index", String(projectIndex));
  if (sessionId) params.set("session_id", sessionId);
  const qs = params.toString();
  return apiFetch<ActiveMemoryResponse>(`/memory/active${qs ? `?${qs}` : ""}`);
}
