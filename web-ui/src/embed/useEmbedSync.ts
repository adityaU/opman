import { useEffect, useMemo, useRef } from "react";
import type { WorkspaceBridge, WorkspaceProject } from "../workspace/DesktopWorkspace";
import { widgetFor } from "../workspace/opener/choices";
import { projectForFile } from "../workspace/fileOpen";
import type { WorkspaceAction } from "../workspace/reducer";
import type { PaneId, WidgetState } from "../workspace/types";
import { widgetKey } from "./embedUrl";
import { postToParent } from "./embedMode";
import { isSameOrigin, readFromParent } from "./messages";

/**
 * The embed's half of the conversation with its parent.
 *
 * Its one pane reports where it is whenever that changes — so a chat that
 * creates its session here is bound on the parent's pane too, and survives the
 * parent reloading — and takes new targets the parent points it at. A target
 * the parent sends is remembered so it is not echoed straight back.
 */
export function useEmbedSync(
  enabled: boolean,
  pane: PaneId,
  widget: WidgetState | null,
  dispatch: (action: WorkspaceAction) => void,
): void {
  const lastKey = useRef<string | null>(widget ? widgetKey(widget) : null);

  useEffect(() => {
    if (!enabled) return;
    const onMessage = (event: MessageEvent) => {
      if (event.source !== window.parent || !isSameOrigin(event)) return;
      const message = readFromParent(event.data);
      if (!message) return;
      const key = widgetKey(message.widget);
      if (key === lastKey.current) return;
      lastKey.current = key;
      dispatch({ type: "openWidget", pane, widget: message.widget });
    };
    window.addEventListener("message", onMessage);
    postToParent({ type: "opman:embed-ready" });
    return () => window.removeEventListener("message", onMessage);
  }, [dispatch, enabled, pane]);

  useEffect(() => {
    if (!enabled || !widget) return;
    const key = widgetKey(widget);
    if (key === lastKey.current) return;
    lastKey.current = key;
    postToParent({ type: "opman:widget-changed", widget });
  }, [enabled, widget]);
}

/**
 * The bridge an embed publishes in place of its own placement: every request
 * to open something goes to the parent, which can see the other panes. The
 * embed is one pane — splitting it would nest a workspace inside a pane.
 */
export function useEmbedBridge(
  embedded: WidgetState | null,
  projects: readonly WorkspaceProject[],
): WorkspaceBridge | null {
  const latest = useRef({ embedded, projects });
  latest.current = { embedded, projects };

  return useMemo(() => {
    if (!embedded) return null;
    const home = () => latest.current.embedded?.projectPath ?? "";
    const send = (widget: WidgetState) => postToParent({ type: "opman:open-widget", widget });
    const openFile = (path: string, line: number | null) =>
      send({
        kind: "files",
        projectPath: projectForFile(path, latest.current.projects, home()),
        sessionId: "embed",
        open: { path, line, seq: 0 },
      });
    return {
      arm: (request) => send(request.widget),
      openKindHere: (kind) => send(widgetFor(kind, home(), "embed" as PaneId)),
      openFile,
      openFileWhere: (path, line) => openFile(path, line),
      openBrowser: (projectPath, url) =>
        send({ ...widgetFor("browser", projectPath, "embed" as PaneId), url } as WidgetState),
    };
    // `embedded` only decides whether there is a bridge at all; what it points
    // at is read through the ref, so the bridge keeps one identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [embedded !== null]);
}
