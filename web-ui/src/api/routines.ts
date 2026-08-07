import { apiFetch, apiPost, apiDelete, apiPatch } from "./client";

// ── Routine types ─────────────────────────────────────

export type RoutineTrigger = "manual" | "scheduled" | "on_session_idle" | "daily_summary";
export type RoutineTargetMode = "existing_session" | "new_session";

export interface RoutineDefinition {
  id: string;
  name: string;
  trigger: RoutineTrigger;
  enabled: boolean;
  cron_expr: string | null;
  timezone: string | null;
  target_mode: RoutineTargetMode | null;
  session_id: string | null;
  project_index: number | null;
  prompt: string | null;
  provider_id: string | null;
  model_id: string | null;
  mission_id: string | null;
  last_run_at: string | null;
  next_run_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface RoutineRunRecord {
  id: string;
  routine_id: string;
  status: string;
  summary: string;
  target_session_id: string | null;
  duration_ms: number | null;
  created_at: string;
}

export interface RoutinesListResponse {
  routines: RoutineDefinition[];
  runs: RoutineRunRecord[];
}

// ── Routine API ───────────────────────────────────────

export async function fetchRoutines(): Promise<RoutinesListResponse> {
  return apiFetch<RoutinesListResponse>("/routines");
}

export async function createRoutine(req: {
  name: string;
  trigger: RoutineTrigger;
  enabled?: boolean;
  cron_expr?: string | null;
  timezone?: string | null;
  target_mode?: RoutineTargetMode | null;
  session_id?: string | null;
  project_index?: number | null;
  prompt?: string | null;
  provider_id?: string | null;
  model_id?: string | null;
  mission_id?: string | null;
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
    enabled?: boolean;
    cron_expr?: string | null;
    timezone?: string | null;
    target_mode?: RoutineTargetMode | null;
    session_id?: string | null;
    project_index?: number | null;
    prompt?: string | null;
    provider_id?: string | null;
    model_id?: string | null;
    mission_id?: string | null;
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

