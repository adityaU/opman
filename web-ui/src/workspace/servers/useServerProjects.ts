import { useCallback, useEffect, useMemo } from "react";
import { SERVER_ID } from "../../api/base";
import { useServers } from "../../hooks/useServers";
import type { WorkspaceProject } from "../DesktopWorkspace";
import type { OpenerChoice } from "../opener/steps";
import {
  findRemoteProject,
  getRemoteCatalog,
  loadServerSessions,
  refreshServerProjects,
  sessionsKey,
  useRemoteCatalog,
} from "./remoteCatalog";
import { groupedProjectChoices, remoteSessionChoices, serverLabel } from "./serverChoices";

/**
 * The opener's view of every server: grouped project rows, and a remote
 * project's sessions.
 *
 * `open` is whether the opener is up. Each opening re-reads the server list and
 * every reachable server's projects, so what the user picks from is what is
 * there now. With one server none of this fetches anything and the project
 * list is the plain one it always was.
 */
export function useServerProjects(projects: readonly WorkspaceProject[], open: boolean) {
  const { servers, multi, refresh } = useServers();
  const catalog = useRemoteCatalog();

  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  // Keyed on ids and statuses, not on the array: `refresh` above hands back a
  // new array every time, and re-reading every server for each would loop.
  const reachable = useMemo(
    () => servers.filter((s) => s.id !== SERVER_ID && s.status === "ok").map((s) => s.id).join("\n"),
    [servers],
  );
  useEffect(() => {
    if (!open || !reachable) return;
    for (const id of reachable.split("\n")) void refreshServerProjects(id);
  }, [open, reachable]);

  const projectChoices = useMemo(
    () => groupedProjectChoices({ id: SERVER_ID, projects }, multi ? servers : [], catalog.projects),
    [catalog.projects, multi, projects, servers],
  );

  const sessionChoices = useCallback(
    (server: string, path: string): readonly OpenerChoice[] =>
      remoteSessionChoices(catalog.sessions.get(sessionsKey(server, path))),
    [catalog.sessions],
  );

  /** Start reading a remote project's sessions; the session step re-renders when they land. */
  const prefetchSessions = useCallback((server: string, path: string) => {
    const project = findRemoteProject(getRemoteCatalog(), server, path);
    if (project) void loadServerSessions(server, project);
  }, []);

  const nameOf = useCallback((id: string) => serverLabel(servers, id), [servers]);

  return { multi, projectChoices, sessionChoices, prefetchSessions, serverName: nameOf };
}
