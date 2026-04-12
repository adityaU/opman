import { apiFetch, apiPost, apiDelete, apiPatch } from "./client";

// ── Routine types ─────────────────────────────────────

export type RoutineTrigger = "manual" | "scheduled" | "on_session_idle" | "daily_summary";
export type RoutineAction = "send_message" | "review_mission" | "open_inbox" | "open_activity_feed";
export type RoutineTargetMode = "existing_session" | "new_session";

export interface RoutineDefinition {
  id: string;
  name: string;
  trigger: RoutineTrigger;
  action: RoutineAction;
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
  action: RoutineAction;
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
    action?: RoutineAction;
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

// ── Delegation Board types ────────────────────────────

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

// ── Delegation API ────────────────────────────────────

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

// ── Workspace Snapshot types ──────────────────────────

export interface WorkspacePanels {
  sidebar: boolean;
  terminal: boolean;
  editor: boolean;
  git: boolean;
}

export interface WorkspaceLayout {
  sidebar_width: number;
  terminal_height: number;
  side_panel_width: number;
}

export interface WorkspaceTerminalTab {
  label: string;
  kind: string;
}

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

export interface WorkspacesListResponse {
  workspaces: WorkspaceSnapshot[];
}

// ── Workspace API ─────────────────────────────────────

export async function fetchWorkspaces(): Promise<WorkspacesListResponse> {
  return apiFetch<WorkspacesListResponse>("/workspaces");
}

export async function saveWorkspace(snapshot: WorkspaceSnapshot): Promise<void> {
  await apiPost("/workspaces", { snapshot });
}

export async function deleteWorkspace(name: string): Promise<void> {
  await apiDelete(`/workspaces?name=${encodeURIComponent(name)}`);
}
