/**
 * What other opman servers have on them: their projects, and a project's
 * sessions once someone asks.
 *
 * One module-level store rather than state in a hook, because three surfaces
 * read it — the opener's project and session steps, and every remote pane's
 * header — and two copies of "what is on the dev box" would disagree. It owns
 * no polling: the opener refreshes it each time it opens, which is the moment
 * the answer matters, and a pane header showing a project name a minute stale
 * is harmless.
 *
 * Reads go through home's proxy (`${serverBase(id)}/api/...`), same origin and
 * same cookie as everything else; home holds the remote's credentials.
 */

import { useSyncExternalStore } from "react";
import { serverBase } from "../../api/base";
import type { AppState, SessionPage } from "../../api/state";

export interface RemoteSession {
  readonly id: string;
  readonly title: string;
  readonly updated: number;
}

export interface RemoteProject {
  readonly path: string;
  readonly name: string;
  /** The project's index on its own server — what `/api/sessions` is keyed by. */
  readonly index: number;
}

export type Load<T> =
  | { readonly status: "loading"; readonly data: T | null }
  | { readonly status: "ok"; readonly data: T }
  | { readonly status: "error"; readonly error: string; readonly data: T | null };

export interface RemoteCatalog {
  /** Keyed by server id. Absent: never asked. */
  readonly projects: ReadonlyMap<string, Load<readonly RemoteProject[]>>;
  /** Keyed by `sessionsKey(server, projectPath)`. */
  readonly sessions: ReadonlyMap<string, Load<readonly RemoteSession[]>>;
}

/** How many sessions the opener lists for a remote project — the newest page. */
const SESSION_PAGE = 50;

let catalog: RemoteCatalog = { projects: new Map(), sessions: new Map() };
const listeners = new Set<() => void>();

function publish(next: RemoteCatalog): void {
  catalog = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getRemoteCatalog(): RemoteCatalog {
  return catalog;
}

export function useRemoteCatalog(): RemoteCatalog {
  return useSyncExternalStore(subscribe, getRemoteCatalog, getRemoteCatalog);
}

export function sessionsKey(server: string, projectPath: string): string {
  return `${server}\u0000${projectPath}`;
}

function setProjects(server: string, load: Load<readonly RemoteProject[]>): void {
  publish({ ...catalog, projects: new Map(catalog.projects).set(server, load) });
}

function setSessions(key: string, load: Load<readonly RemoteSession[]>): void {
  publish({ ...catalog, sessions: new Map(catalog.sessions).set(key, load) });
}

/** A failure in the words the opener shows beside the server's name. */
function describeFailure(status: number): string {
  if (status === 401 || status === 403) return "sign-in failed";
  if (status === 404) return "not found";
  if (status >= 500) return "unreachable";
  return `error ${status}`;
}

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url, { credentials: "same-origin" });
  if (!res.ok) throw new Error(describeFailure(res.status));
  return (await res.json()) as T;
}

function errorText(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "unreachable";
}

/**
 * Re-read a server's projects. What was known keeps showing while the read is
 * in flight, so reopening the opener does not flash every group to "loading".
 */
export async function refreshServerProjects(server: string): Promise<void> {
  const previous = catalog.projects.get(server)?.data ?? null;
  setProjects(server, { status: "loading", data: previous });
  try {
    const state = await getJson<AppState>(`${serverBase(server)}/api/state`);
    const projects = (state.projects ?? []).map((project, position) => ({
      path: project.path,
      name: project.name,
      index: typeof project.index === "number" ? project.index : position,
    }));
    setProjects(server, { status: "ok", data: projects });
  } catch (error) {
    setProjects(server, { status: "error", error: errorText(error), data: previous });
  }
}

/** Read a remote project's newest sessions, most recently touched first. */
export async function loadServerSessions(server: string, project: RemoteProject): Promise<void> {
  const key = sessionsKey(server, project.path);
  const previous = catalog.sessions.get(key)?.data ?? null;
  setSessions(key, { status: "loading", data: previous });
  const params = new URLSearchParams({
    project: String(project.index),
    offset: "0",
    limit: String(SESSION_PAGE),
  });
  try {
    const page = await getJson<SessionPage>(`${serverBase(server)}/api/sessions?${params}`);
    const sessions = page.sessions
      .map((session) => ({
        id: session.id,
        title: session.title || session.id.slice(0, 8),
        updated: session.time?.updated ?? 0,
      }))
      .sort((a, b) => b.updated - a.updated);
    setSessions(key, { status: "ok", data: sessions });
  } catch (error) {
    setSessions(key, { status: "error", error: errorText(error), data: previous });
  }
}

/** The project at `path` on `server`, if that server's list has been read. */
export function findRemoteProject(
  snapshot: RemoteCatalog,
  server: string,
  path: string,
): RemoteProject | null {
  return snapshot.projects.get(server)?.data?.find((project) => project.path === path) ?? null;
}

/** Tests only: forget everything. */
export function resetRemoteCatalog(): void {
  publish({ projects: new Map(), sessions: new Map() });
}
