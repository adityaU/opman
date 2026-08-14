import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { fetchSessionPage, type ProjectInfo, type SessionInfo } from "../api";

/** Top-level sessions fetched per "show more". Matches the server's page size. */
const PAGE = 20;

type Extras = Map<number, SessionInfo[]>;

function mergeSessions(fresh: SessionInfo[], extra: SessionInfo[]): SessionInfo[] {
  if (extra.length === 0) return fresh;
  // `fresh` comes from /api/state on every state change, so it wins on conflict —
  // an extra page is a one-shot read that goes stale the moment a title changes.
  const byId = new Map(extra.map((s) => [s.id, s]));
  for (const session of fresh) byId.set(session.id, session);
  return [...byId.values()];
}

/**
 * Server-side paging for the sidebar's session list.
 *
 * `/api/state` ships only the newest page plus a total. This holds the pages the
 * user has since opened, merges them into the project the sidebar renders, and
 * re-fetches pinned or open sessions by id so a favourite from months ago does
 * not disappear just because it fell off page one.
 */
export function useSessionPages(
  projects: ProjectInfo[],
  activeProject: number,
  keepIds: Set<string>,
) {
  const [extras, setExtras] = useState<Extras>(() => new Map());
  const [loading, setLoading] = useState(false);
  const hydrated = useRef<string>("");

  const project = projects[activeProject] as ProjectInfo | undefined;

  const addSessions = useCallback((index: number, sessions: SessionInfo[]) => {
    if (sessions.length === 0) return;
    setExtras((prev) => {
      const next = new Map(prev);
      next.set(index, mergeSessions(sessions, prev.get(index) ?? []));
      return next;
    });
  }, []);

  // Pinned and open rows are client-only state the server cannot page around, so
  // ask for them by id once per distinct set.
  useEffect(() => {
    if (!project) return;
    const known = new Set(project.sessions.map((s) => s.id));
    const missing = [...keepIds].filter((id) => !known.has(id));
    if (missing.length === 0) return;
    const key = `${activeProject}:${missing.sort().join(",")}`;
    if (hydrated.current === key) return;
    hydrated.current = key;
    let live = true;
    fetchSessionPage(activeProject, { ids: missing })
      .then((page) => live && addSessions(activeProject, page.sessions))
      .catch(() => {
        // A failed hydrate must not wedge the sidebar; the next set retries.
        hydrated.current = "";
      });
    return () => {
      live = false;
    };
  }, [project, activeProject, keepIds, addSessions]);

  const merged = useMemo(() => {
    if (!project) return project;
    const extra = extras.get(activeProject);
    if (!extra || extra.length === 0) return project;
    return { ...project, sessions: mergeSessions(project.sessions, extra) };
  }, [project, extras, activeProject]);

  const loadedParents = useMemo(
    () => (merged?.sessions ?? []).filter((s) => !s.parentID).length,
    [merged],
  );

  const total = project?.session_count ?? loadedParents;
  const remaining = Math.max(0, total - loadedParents);

  const loadMore = useCallback(async () => {
    if (loading || remaining === 0) return;
    setLoading(true);
    try {
      const page = await fetchSessionPage(activeProject, {
        offset: loadedParents,
        limit: PAGE,
      });
      addSessions(activeProject, page.sessions);
    } catch {
      // Leave the button in place — the list the user already has stays valid.
    } finally {
      setLoading(false);
    }
  }, [loading, remaining, activeProject, loadedParents, addSessions]);

  // The whole array, so every consumer resolves a session id against the same
  // list — Open Sessions looks rows up here too, and an open tab from last month
  // would otherwise be unresolvable.
  const pagedProjects = useMemo(
    () =>
      merged === project
        ? projects
        : projects.map((p, i) => (i === activeProject ? (merged as ProjectInfo) : p)),
    [projects, merged, project, activeProject],
  );

  return { projects: pagedProjects, project: merged, remaining, loading, loadMore };
}
