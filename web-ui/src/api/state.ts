import { apiFetch, apiPatch, apiPost } from "./client";

// ── Types ─────────────────────────────────────────────

/**
 * What a session is configured to run as, as its runner reports it.
 *
 * Not opman's to store: every runner already keeps these per session and persists them,
 * because each has to reproduce the same configuration when it resumes a conversation.
 * A field is absent when the session has never had that choice made — which a runner
 * answers with its own current default, and is not the same as being pinned to one.
 */
export interface EngineChoices {
  model?: string;
  agent?: string;
  effort?: string;
  permissionMode?: string;
}

export interface SessionInfo {
  id: string;
  title: string;
  parentID: string;
  directory: string;
  time: { created: number; updated: number };
  runner?: string;
  engine?: EngineChoices;
}

export interface ProjectInfo {
  name: string;
  path: string;
  index: number;
  active_session: string | null;
  sessions: SessionInfo[];
  git_branch: string;
  busy_sessions: string[];
}

export interface AppState {
  /** Backend has completed the first session hydration for every project. */
  startup_ready?: boolean;
  projects: ProjectInfo[];
  active_project: number;
  panels: PanelVisibility;
  focused: string;
  /** Optional instance name from tunnel hostname, used as page title. */
  instance_name?: string;
  /**
   * CLI opman wraps: "opencode" or "claude-code". Both claude engines report
   * "claude-code", so this cannot identify a runner — use `default_runner`.
   */
  backend?: string;
  /** Runner that owns sessions with no runner of their own. */
  default_runner?: string;
  runners?: string[];
}

export interface PanelVisibility {
  sidebar: boolean;
  terminal_pane: boolean;
  neovim_pane: boolean;
  integrated_terminal: boolean;
  git_panel: boolean;
}

export interface SessionStats {
  session_id?: string;
  cost: number;
  input_tokens: number;
  output_tokens: number;
  reasoning_tokens: number;
  cache_read: number;
  cache_write: number;
}

/** Theme colors — 15 hex strings matching the TUI's ThemeColors struct. */
export interface ThemeColors {
  primary: string;
  secondary: string;
  accent: string;
  background: string;
  background_panel: string;
  background_element: string;
  text: string;
  text_muted: string;
  border: string;
  border_active: string;
  border_subtle: string;
  error: string;
  warning: string;
  success: string;
  info: string;
}

// ── Auth ──────────────────────────────────────────────

export async function login(
  username: string,
  password: string
): Promise<string> {
  const res = await fetch("/api/auth/login", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password }),
  });
  if (!res.ok) throw new Error("Invalid credentials");
  // The backend sets the auth cookie via Set-Cookie header.
  // We still return the token from the JSON body for backward compat.
  const data = await res.json();
  return data.token;
}

export async function verifyToken(): Promise<boolean> {
  // With cookie auth the browser automatically sends the opman_token
  // cookie — no need to check sessionStorage first.
  try {
    const res = await fetch("/api/auth/verify", {
      credentials: "same-origin",
    });
    return res.ok;
  } catch {
    return false;
  }
}

// ── State fetchers ────────────────────────────────────

export async function fetchAppState(): Promise<AppState> {
  return apiFetch<AppState>("/state");
}

export async function fetchSessionStats(
  sessionId: string
): Promise<SessionStats | null> {
  try {
    return await apiFetch<SessionStats>(`/session/${sessionId}/stats`);
  } catch {
    return null;
  }
}

/** Backend theme pair: both dark and light variants. */
export interface ThemePair {
  /** Active theme name — what the picker needs to mark the current palette. */
  name: string;
  dark: ThemeColors;
  light: ThemeColors;
}

export async function fetchThemePair(): Promise<ThemePair | null> {
  try {
    return await apiFetch<ThemePair>("/theme");
  } catch {
    return null;
  }
}

// ── Themes ────────────────────────────────────────────

/** Theme preview with both dark and light color sets. */
/**
 * Tell the session's runner what it should run as, without sending a turn.
 *
 * A send already carries these, which covers a choice the user makes and then acts on.
 * This covers the one they make and leave: nothing was sent, so without it the pick was
 * never recorded anywhere and the session came back configured as it last ran.
 *
 * Only the fields present are applied — one chip's change must not clear the other three.
 * `accepted` is false for a runner with no way to be configured out of band; that is not
 * an error, the choice simply applies with the next turn as it always did.
 */
export async function setSessionEngine(
  sessionId: string,
  choices: EngineChoices,
): Promise<boolean> {
  const result = await apiPatch<{ accepted?: boolean }>(
    `/session/${sessionId}/engine`,
    choices,
  );
  return result?.accepted === true;
}

export interface ThemePreview {
  name: string;
  dark: ThemeColors;
  light: ThemeColors;
}

/** Fetch all available themes with both variants */
export async function fetchThemes(): Promise<ThemePreview[]> {
  return apiFetch<ThemePreview[]>("/themes");
}

/** Switch the active theme by name. Returns the full ThemePair. */
export async function switchTheme(name: string): Promise<ThemePair> {
  return apiPost<ThemePair>("/theme/switch", { name });
}

// ── Public (unauthenticated) endpoints ────────────────

/** Bootstrap data returned before authentication. */
export interface BootstrapData {
  theme: ThemePair | null;
  instance_name: string | null;
}

/** Fetch public bootstrap data (theme + instance name) without auth.
 *  Used on the login page so the form renders with the active theme. */
export async function fetchBootstrap(): Promise<BootstrapData> {
  try {
    const res = await fetch("/api/public/bootstrap");
    if (!res.ok) return { theme: null, instance_name: null };
    const raw: BootstrapData = await res.json();
    return raw;
  } catch {
    return { theme: null, instance_name: null };
  }
}
