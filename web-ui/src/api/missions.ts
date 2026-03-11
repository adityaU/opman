import { apiFetch, apiPost, apiDelete, apiPatch } from "./client";

// ── Mission types ─────────────────────────────────────

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

// ── Mission API ───────────────────────────────────────

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
