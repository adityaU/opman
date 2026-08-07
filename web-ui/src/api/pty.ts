import { apiFetch, apiPost } from "./client";

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

/** What a PTY's terminal is doing, as reported by its foreground process group. */
export type PtyActivity = "idle" | "running";

/**
 * Which PTYs are running a foreground command, keyed by id.
 *
 * A PTY that has been killed is absent from the map rather than reported idle.
 */
export async function ptyActivity(): Promise<Record<string, PtyActivity>> {
  return apiFetch<Record<string, PtyActivity>>("/pty/activity");
}

/**
 * Open the output stream for a PTY.
 *
 * `replay` asks the server to lead with the PTY's retained scrollback, which is
 * how a tab that re-attached to a surviving PTY repaints what is on screen.
 * A freshly spawned PTY must not replay — it would repaint history it never had.
 */
export function createPtySSE(id: string, replay = false): EventSource {
  // Cookie auth: browser sends opman_token cookie automatically.
  const query = `id=${encodeURIComponent(id)}${replay ? "&replay=1" : ""}`;
  return new EventSource(`/api/pty/stream?${query}`);
}

// ── App events SSE ────────────────────────────────────

export function createEventsSSE(): EventSource {
  // Cookie auth: browser sends opman_token cookie automatically.
  return new EventSource(`/api/events`);
}
