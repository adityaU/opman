/**
 * The opener's project and session lists once there is more than one server.
 *
 * Pure, so the grouping rule is testable without rendering an opener: every
 * server is a group, this instance's own first; a server that cannot answer is
 * listed rather than hidden, as one disabled row saying why — a server missing
 * from the list reads as "it was never added", which sends the user to the
 * wrong place to fix it.
 */

import type { ServerInfo } from "../../api/servers";
import { formatTime } from "../../sidebar/formatTime";
import type { WorkspaceProject } from "../DesktopWorkspace";
import { projectChoiceValue, type OpenerChoice } from "../opener/steps";
import type { Load, RemoteProject, RemoteSession } from "./remoteCatalog";

const STATUS_TEXT: Readonly<Record<ServerInfo["status"], string>> = {
  ok: "",
  unreachable: "unreachable",
  auth_failed: "sign-in failed",
};

export interface OwnServer {
  readonly id: string;
  readonly projects: readonly WorkspaceProject[];
}

/** A server's display name, or its id while the list has not loaded. */
export function serverLabel(servers: readonly ServerInfo[], id: string): string {
  return servers.find((server) => server.id === id)?.name ?? id;
}

/**
 * Projects of every server, grouped. With one server this is exactly the old
 * flat list — same values, no headings — so nothing changes until a second
 * server exists.
 */
export function groupedProjectChoices(
  own: OwnServer,
  servers: readonly ServerInfo[],
  remote: ReadonlyMap<string, Load<readonly RemoteProject[]>>,
): readonly OpenerChoice[] {
  const flat = own.projects.map((p) => ({ value: p.path, label: p.name, hint: p.path }));
  const others = servers.filter((server) => server.id !== own.id);
  if (others.length === 0) return flat;

  const ownGroup = serverLabel(servers, own.id);
  return [
    ...flat.map((choice) => ({ ...choice, group: ownGroup, groupNote: "this server" })),
    ...others.flatMap((server) => remoteGroup(server, remote.get(server.id))),
  ];
}

function remoteGroup(
  server: ServerInfo,
  load: Load<readonly RemoteProject[]> | undefined,
): readonly OpenerChoice[] {
  const group = server.name;
  const statusRow = (label: string, note: string): OpenerChoice => ({
    value: projectChoiceValue(server.id, ""),
    label,
    hint: server.url ?? undefined,
    group,
    groupNote: note,
    disabled: true,
  });

  // Home already knows the server is down; asking it again only delays saying so.
  if (server.status !== "ok") return [statusRow("Not available", STATUS_TEXT[server.status])];
  if (!load || (load.status === "loading" && !load.data)) return [statusRow("Loading projects…", "")];
  if (load.status === "error" && !load.data) return [statusRow("Not available", load.error)];

  const projects = load.data ?? [];
  if (projects.length === 0) return [statusRow("No projects", "")];
  const note = load.status === "error" ? load.error : "";
  return projects.map((project) => ({
    value: projectChoiceValue(server.id, project.path),
    label: project.name,
    hint: project.path,
    group,
    groupNote: note,
    disabled: load.status === "error",
  }));
}

/** "New session" over a remote project's sessions, or a row saying they are on their way. */
export function remoteSessionChoices(
  load: Load<readonly RemoteSession[]> | undefined,
): readonly OpenerChoice[] {
  const fresh: OpenerChoice = { value: null, label: "New session", hint: "created on first send" };
  if (!load || (load.status === "loading" && !load.data)) {
    return [fresh, { value: "\u0000loading", label: "Loading sessions…", disabled: true }];
  }
  if (load.status === "error" && !load.data) {
    return [fresh, { value: "\u0000error", label: `Sessions ${load.error}`, disabled: true }];
  }
  return [
    fresh,
    ...(load.data ?? []).map((s) => ({ value: s.id, label: s.title, hint: formatTime(s.updated) })),
  ];
}

/**
 * The shell step for another server's project. Its shells are that server's,
 * listed by the pane itself once it is showing — so the only answer to give
 * here is "open it", and the embedded terminal asks which shell.
 */
export const REMOTE_SHELL_CHOICES: readonly OpenerChoice[] = [
  { value: null, label: "Choose in the pane", hint: "lists that server's shells" },
];
