import { apiFetch, apiPost, getToken } from "./client";

// ── Types ─────────────────────────────────────────────

export interface SpawnPtyResponse {
  id: string;
  ok: boolean;
}

// ── PTY management ────────────────────────────────────

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

// ── App events SSE ────────────────────────────────────

export function createEventsSSE(): EventSource {
  const token = getToken();
  return new EventSource(
    `/api/events?token=${encodeURIComponent(token || "")}`
  );
}
