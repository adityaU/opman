import { apiFetch, apiPost } from "./client";

// ── Types ─────────────────────────────────────────────

export interface DirEntry {
  name: string;
  path: string;
  /** Whether this path is already added as a project. */
  is_project: boolean;
}

export interface BrowseDirsResponse {
  path: string;
  parent: string;
  entries: DirEntry[];
}

export interface NewSessionResponse {
  session_id: string;
}

// ── Project actions ───────────────────────────────────

export async function switchProject(index: number): Promise<void> {
  return apiPost("/project/switch", { index });
}

/** Add a new project by directory path. Returns the new project index and name. */
export async function addProject(
  path: string,
  name?: string
): Promise<{ index: number; name: string }> {
  return apiPost("/project/add", { path, name });
}

/** Remove a project by index */
export async function removeProject(index: number): Promise<void> {
  return apiPost("/project/remove", { index });
}

// ── Directory browsing (for add-project picker) ───────

/** Browse subdirectories of a given path. */
export async function browseDirs(
  path: string
): Promise<BrowseDirsResponse> {
  return apiPost("/dirs/browse", { path });
}

/** Get the user's home directory. */
export async function getHomeDir(): Promise<{ path: string }> {
  return apiFetch("/dirs/home");
}

// ── Session selection ─────────────────────────────────

export async function selectSession(
  projectIdx: number,
  sessionId: string
): Promise<void> {
  return apiPost("/session/select", {
    project_idx: projectIdx,
    session_id: sessionId,
  });
}

export async function newSession(
  projectIdx: number
): Promise<NewSessionResponse> {
  return apiPost("/session/new", { project_idx: projectIdx });
}

// ── Panel actions ─────────────────────────────────────

export async function togglePanel(panel: string): Promise<void> {
  return apiPost("/panel/toggle", { panel });
}

export async function focusPanel(panel: string): Promise<void> {
  return apiPost("/panel/focus", { panel });
}
