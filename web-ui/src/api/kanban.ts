import { apiFetch, apiPost, apiPatch, apiDelete, apiPut } from "./client";

// ── Types (mirror the backend contract exactly) ───────────────────────

export interface Lane {
  id: string;
  name: string;
  color: string;
  wip: number | null;
  terminal: boolean;
  agent: string | null;
  model: string | null;
}

/** Adjacency list: laneId -> allowed target laneIds. */
export type Transitions = Record<string, string[]>;

export interface Board {
  id: string;
  name: string;
  project_path: string;
  lanes: Lane[];
  transitions: Transitions;
}

export interface Attachment {
  id: string;
  filename: string;
  mime: string;
  kind: "image" | "video" | "file";
  size_bytes: number;
  url: string;
}

export interface Note {
  id: string;
  author: "agent" | "user";
  body: string;
  lane_from: string | null;
  lane_to: string | null;
  created_at: string;
}

export type Priority = "low" | "normal" | "high" | "urgent";
export type RunState = "idle" | "launching" | "running" | "done" | "failed";

export interface Task {
  id: string;
  board_id: string;
  lane_id: string;
  title: string;
  description: string;
  tags: string[];
  priority: Priority;
  order_index: number;
  session_id: string | null;
  launch_model: string | null;
  launch_agent: string | null;
  run_state: RunState;
  created_at: string;
  updated_at: string;
}

export interface TaskDetail extends Task {
  notes: Note[];
  attachments: Attachment[];
}

export interface BoardResponse {
  board: Board;
  tasks: Task[];
}

// ── Board ──────────────────────────────────────────────────────────────

export async function fetchBoard(projectIndex: number): Promise<BoardResponse> {
  return apiFetch<BoardResponse>(`/kanban/board?pi=${projectIndex}`);
}

export async function saveBoardConfig(
  boardId: string,
  lanes: Lane[],
  transitions: Transitions,
): Promise<{ board: Board }> {
  return apiPut<{ board: Board }>(`/kanban/board/${boardId}/config`, { lanes, transitions });
}

// ── Tasks ────────────────────────────────────────────────────────────────

export interface CreateTaskInput {
  board_id: string;
  lane_id: string;
  title: string;
  description: string;
  tags: string[];
  priority: Priority;
}

export async function createTask(input: CreateTaskInput): Promise<Task> {
  return apiPost<Task>(`/kanban/task`, input);
}

export interface PatchTaskInput {
  title?: string;
  description?: string;
  tags?: string[];
  priority?: Priority;
  lane_id?: string;
  order_index?: number;
}

export async function patchTask(taskId: string, input: PatchTaskInput): Promise<Task> {
  return apiPatch<Task>(`/kanban/task/${taskId}`, input);
}

export async function deleteTask(taskId: string): Promise<void> {
  return apiDelete(`/kanban/task/${taskId}`);
}

export async function fetchTaskDetail(taskId: string): Promise<TaskDetail> {
  return apiFetch<TaskDetail>(`/kanban/task/${taskId}`);
}

// ── Attachments ────────────────────────────────────────────────────────

export async function uploadAttachment(taskId: string, file: File): Promise<Attachment> {
  const form = new FormData();
  form.append("file", file);
  const res = await fetch(`/api/kanban/task/${taskId}/attachment`, {
    method: "POST",
    credentials: "same-origin",
    body: form,
  });
  if (res.status === 401) {
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) {
    const raw = await res.text().catch(() => "");
    let detail = "";
    try { const j = JSON.parse(raw); detail = j.error || j.message || ""; } catch { detail = raw; }
    throw new Error(detail || `HTTP ${res.status}`);
  }
  return res.json() as Promise<Attachment>;
}

/** Build the direct asset URL usable in <img>/<video> src. */
export function assetUrl(taskId: string, filename: string): string {
  return `/api/kanban/asset/${taskId}/${encodeURIComponent(filename)}`;
}

// ── Launch / Abort ─────────────────────────────────────────────────────

export async function launchTask(
  taskId: string,
  body: { model?: string; agent?: string },
): Promise<{ session_id: string }> {
  return apiPost<{ session_id: string }>(`/kanban/task/${taskId}/launch`, body);
}

export async function abortTask(taskId: string): Promise<void> {
  return apiPost(`/kanban/task/${taskId}/abort`);
}
