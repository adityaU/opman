/**
 * Frontend API client for backend-computed intelligence endpoints.
 *
 * These functions call the Rust backend which owns all derived/business
 * logic. The frontend passes only transient SSE data (permissions,
 * questions, watcher status, signals) and the backend merges it with
 * its own persisted state to compute results.
 */

import { apiFetch, apiPost } from "./client";

// ── Shared input types (match Rust serde shapes) ─────────

export interface PermissionInput {
  id: string;
  sessionID: string;
  toolName: string;
  description?: string;
  time: number;
}

export interface QuestionInput {
  id: string;
  sessionID: string;
  title: string;
  time: number;
}

export interface SignalInput {
  id: string;
  kind: string;
  title: string;
  body: string;
  created_at: number;
  session_id?: string | null;
}

export interface WatcherStatusInput {
  session_id: string;
  action: string;
  idle_since_secs?: number;
}

// ── Inbox ────────────────────────────────────────────────

export type InboxItemPriority = "high" | "medium" | "low";
export type InboxItemState = "unresolved" | "informational";
export type InboxItemSource = "permission" | "question" | "mission" | "watcher" | "completion";

export interface InboxItem {
  id: string;
  source: InboxItemSource;
  title: string;
  description: string;
  priority: InboxItemPriority;
  state: InboxItemState;
  created_at: number;
  session_id?: string | null;
  mission_id?: string | null;
}

export interface InboxResponse {
  items: InboxItem[];
}

export async function computeInbox(req: {
  permissions: PermissionInput[];
  questions: QuestionInput[];
  watcher_status?: WatcherStatusInput | null;
  signals: SignalInput[];
}): Promise<InboxResponse> {
  return apiPost<InboxResponse>("/inbox", req);
}

// ── Recommendations ──────────────────────────────────────

export type RecommendationAction =
  | "open_inbox"
  | "open_missions"
  | "open_memory"
  | "open_routines"
  | "open_delegation"
  | "open_workspaces"
  | "open_autonomy"
  | "setup_daily_summary"
  | "upgrade_autonomy_nudge"
  | "setup_daily_copilot";

export interface AssistantRecommendation {
  id: string;
  title: string;
  rationale: string;
  action: RecommendationAction;
  priority: InboxItemPriority;
}

export interface RecommendationsResponse {
  recommendations: AssistantRecommendation[];
}

export async function computeRecommendations(req: {
  permissions: PermissionInput[];
  questions: QuestionInput[];
}): Promise<RecommendationsResponse> {
  return apiPost<RecommendationsResponse>("/recommendations", req);
}

// ── Handoffs ─────────────────────────────────────────────

export interface HandoffLink {
  kind: string;
  label: string;
  source_id?: string | null;
}

export interface HandoffBrief {
  title: string;
  summary: string;
  blockers: string[];
  recent_changes: string[];
  next_action: string;
  links: HandoffLink[];
}

export async function computeMissionHandoff(req: {
  mission_id: string;
  permissions: PermissionInput[];
  questions: QuestionInput[];
}): Promise<HandoffBrief | null> {
  try {
    return await apiPost<HandoffBrief>("/handoff/mission", req);
  } catch {
    return null;
  }
}

export async function computeSessionHandoff(req: {
  session_id: string;
  permissions: PermissionInput[];
  questions: QuestionInput[];
}): Promise<HandoffBrief | null> {
  try {
    return await apiPost<HandoffBrief>("/handoff/session", req);
  } catch {
    return null;
  }
}

// ── Resume Briefing ──────────────────────────────────────

export interface ResumeBriefing {
  title: string;
  summary: string;
  next_action: string;
}

export async function computeResumeBriefing(req: {
  active_session_id?: string | null;
  permissions: PermissionInput[];
  questions: QuestionInput[];
  signals: SignalInput[];
}): Promise<ResumeBriefing | null> {
  try {
    return await apiPost<ResumeBriefing>("/resume-briefing", req);
  } catch {
    return null;
  }
}

// ── Daily Summary ────────────────────────────────────────

export interface DailySummaryResponse {
  summary: string;
}

export async function computeDailySummary(req: {
  routine_id: string;
  permissions: PermissionInput[];
  questions: QuestionInput[];
  signals: SignalInput[];
}): Promise<DailySummaryResponse> {
  return apiPost<DailySummaryResponse>("/daily-summary", req);
}

// ── Signals ──────────────────────────────────────────────

export interface SignalsResponse {
  signals: SignalInput[];
}

export async function fetchSignals(): Promise<SignalsResponse> {
  return apiFetch<SignalsResponse>("/signals");
}

export async function addSignal(req: {
  kind: string;
  title: string;
  body: string;
  session_id?: string | null;
}): Promise<SignalInput> {
  return apiPost<SignalInput>("/signals", req);
}

// ── Assistant Center Stats ───────────────────────────────

export interface AssistantCenterStats {
  active_missions: number;
  blocked_missions: number;
  total_missions: number;
  pending_permissions: number;
  pending_questions: number;
  memory_items: number;
  active_routines: number;
  active_delegations: number;
  workspace_count: number;
  autonomy_mode: string;
}

export async function computeAssistantStats(req: {
  permissions: PermissionInput[];
  questions: QuestionInput[];
}): Promise<AssistantCenterStats> {
  return apiPost<AssistantCenterStats>("/assistant-center/stats", req);
}

// ── Workspace Templates ──────────────────────────────────

export interface WorkspaceTemplate {
  id: string;
  name: string;
  description: string;
  panels: { sidebar: boolean; terminal: boolean; editor: boolean; git: boolean };
  layout: { sidebar_width: number; terminal_height: number; side_panel_width: number };
}

export interface WorkspaceTemplatesResponse {
  templates: WorkspaceTemplate[];
}

export async function fetchWorkspaceTemplates(): Promise<WorkspaceTemplatesResponse> {
  return apiFetch<WorkspaceTemplatesResponse>("/workspace-templates");
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

// ── Routine Summary (backend-computed) ───────────────────

export async function computeRoutineSummary(req: {
  routine_id: string;
  active_session_id?: string | null;
  permissions?: PermissionInput[];
  questions?: QuestionInput[];
  signals?: SignalInput[];
}): Promise<DailySummaryResponse> {
  return apiPost<DailySummaryResponse>("/daily-summary", {
    routine_id: req.routine_id,
    permissions: req.permissions ?? [],
    questions: req.questions ?? [],
    signals: req.signals ?? [],
  });
}
