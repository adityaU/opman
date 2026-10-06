import { useCallback } from "react";
import { useServers } from "../../hooks/useServers";
import { basename } from "../../utils/path";
import { isForeign } from "../foreignOpen";
import { targetLabel } from "../history";
import type { WidgetState } from "../types";
import type { PaneContext } from "../WorkspaceRoot";
import { findRemoteProject, sessionsKey, useRemoteCatalog } from "./remoteCatalog";
import { serverLabel } from "./serverChoices";

/**
 * A remote pane's chrome: the server's name, the project's name as that server
 * calls it, and what the pane is on. Null for this instance's own widgets.
 *
 * Read from the shared catalog, so it is as fresh as the last time the opener
 * looked; before then the project falls back to its folder name. Busy is the
 * embed's to show — its own pane draws the rim inside the frame.
 */
export function useRemoteDescribe(): (widget: WidgetState) => PaneContext | null {
  const { servers } = useServers();
  const catalog = useRemoteCatalog();

  return useCallback(
    (widget: WidgetState): PaneContext | null => {
      if (!widget.server || !isForeign(widget)) return null;
      const server = widget.server;
      const project = findRemoteProject(catalog, server, widget.projectPath);
      const session = widget.kind === "chat" && widget.sessionId
        ? catalog.sessions
          .get(sessionsKey(server, widget.projectPath))
          ?.data?.find((candidate) => candidate.id === widget.sessionId)
        : undefined;
      return {
        projectName: project?.name ?? basename(widget.projectPath),
        subtitle: session?.title ?? (widget.kind === "git" ? null : targetLabel(widget)),
        busy: false,
        serverName: serverLabel(servers, server),
      };
    },
    [catalog, servers],
  );
}
