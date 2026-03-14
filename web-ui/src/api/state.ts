import { apiFetch, apiPost } from "./client";

// ── Types ─────────────────────────────────────────────

export interface SessionInfo {
  id: string;
  title: string;
  parentID: string;
  directory: string;
  time: { created: number; updated: number };
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
  projects: ProjectInfo[];
  active_project: number;
  panels: PanelVisibility;
  focused: string;
  /** Optional instance name from tunnel hostname, used as page title. */
  instance_name?: string;
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

export async function fetchTheme(): Promise<ThemeColors | null> {
  try {
    return await apiFetch<ThemeColors>("/theme");
  } catch {
    return null;
  }
}

// ── Themes ────────────────────────────────────────────

/** Theme preview with resolved colors */
export interface ThemePreview {
  name: string;
  colors: ThemeColors;
}

/** Fetch all available themes with preview colors */
export async function fetchThemes(): Promise<ThemePreview[]> {
  return apiFetch<ThemePreview[]>("/themes");
}

/** Switch the active theme by name. Returns the new theme colors. */
export async function switchTheme(name: string): Promise<ThemeColors> {
  return apiPost<ThemeColors>("/theme/switch", { name });
}

// ── Public (unauthenticated) endpoints ────────────────

/** Bootstrap data returned before authentication. */
export interface BootstrapData {
  theme: ThemeColors | null;
  instance_name: string | null;
}

/** Fetch public bootstrap data (theme + instance name) without auth.
 *  Used on the login page so the form renders with the active theme. */
export async function fetchBootstrap(): Promise<BootstrapData> {
  try {
    const res = await fetch("/api/public/bootstrap");
    if (!res.ok) return { theme: null, instance_name: null };
    return await res.json();
  } catch {
    return { theme: null, instance_name: null };
  }
}
