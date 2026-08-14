import { apiFetch, apiPost } from "./client";

// ── Types ─────────────────────────────────────────────

export interface SpawnPtyResponse {
  id: string;
  ok: boolean;
}

/** What a PTY is running. The wire names the server accepts. */
export type PtyKind = "shell" | "neovim" | "git" | "opencode" | "claude-attach";

/** What a PTY's terminal is doing, as reported by its foreground process group. */
export type PtyActivity = "idle" | "running";

/**
 * One shell running in the server process.
 *
 * A shell outlives every pane that shows it: closing a terminal, zooming a
 * pane, moving a widget to another window or reloading the browser all detach
 * the view and leave the program running. So the server's list — not any pane's
 * memory — is what the shell picker is a view of.
 */
export interface PtySession {
  readonly id: string;
  readonly kind: PtyKind;
  readonly label: string;
  /** Absolute path of the project the shell was started in. */
  readonly project: string;
  readonly activity: PtyActivity;
}

export interface SpawnPtyOptions {
  /** Which project to start in. Sent explicitly: the server's fallback is the
   *  globally active project, which is the wrong answer for a pane. */
  readonly project?: string | null;
  /** Leave unset to have the server number it within the project. */
  readonly label?: string;
  readonly sessionId?: string;
}

// ── PTY management ────────────────────────────────────

/**
 * Start a PTY under a client-chosen id.
 *
 * Safe to call for an id that is already live — the server returns the running
 * PTY rather than starting a second program over it.
 */
export async function spawnPty(
  kind: PtyKind,
  id: string,
  rows: number,
  cols: number,
  options: SpawnPtyOptions = {},
): Promise<SpawnPtyResponse> {
  const body: Record<string, unknown> = { kind, id, rows, cols };
  if (options.project) body.project = options.project;
  if (options.label) body.label = options.label;
  if (options.sessionId) body.session_id = options.sessionId;
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

/** Rename a shell as the picker lists it. The program is untouched. */
export async function ptyRename(id: string, label: string): Promise<void> {
  return apiPost("/pty/rename", { id, label });
}

/** End a shell. The only thing that does, short of the program exiting. */
export async function ptyKill(id: string): Promise<void> {
  return apiPost("/pty/kill", { id });
}

/**
 * Every live shell, newest state each call.
 *
 * Programs that have exited are dropped as part of answering, so an id absent
 * from the result is genuinely gone rather than merely quiet.
 */
export async function ptySessions(): Promise<PtySession[]> {
  return apiFetch<PtySession[]>("/pty/sessions");
}

/**
 * Open the output stream for a PTY.
 *
 * `replay` asks the server to lead with the PTY's retained scrollback, which is
 * how a view that attached to a running shell repaints what is on screen.
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
