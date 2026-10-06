import { useEffect } from "react";
import { useEmbedBridge, useEmbedSync } from "../embed/useEmbedSync";
import type { WorkspaceBridge, WorkspaceProject } from "./DesktopWorkspace";
import type { WorkspaceAction } from "./reducer";
import type { PaneNode, WidgetState } from "./types";
import { useEmbedRequests } from "./useEmbedRequests";

/**
 * How the workspace talks to what is outside it.
 *
 * Normally that is the shell: a sidebar click, a tool card's file link, an MCP
 * event, each reaching the placement planners through the published bridge —
 * plus requests from embedded panes of other servers. Inside an embed it is the
 * parent instead: the bridge forwards everything there, and the lone pane keeps
 * the parent told of where it is.
 */

export interface BridgeDeps {
  readonly embed: WidgetState | null;
  readonly projects: readonly WorkspaceProject[];
  readonly focusedPane: PaneNode | null;
  readonly dispatch: (action: WorkspaceAction) => void;
  readonly placement: WorkspaceBridge & { readonly openForeignWidget: (widget: WidgetState) => void };
  readonly targetingBridge?: (api: WorkspaceBridge | null) => void;
}

export function useWorkspaceBridge(deps: BridgeDeps): void {
  const { dispatch, embed, focusedPane, placement, projects, targetingBridge } = deps;
  const embedded = embed !== null;

  const forwarding = useEmbedBridge(embed, projects);
  useEmbedSync(embedded, focusedPane?.id ?? ("" as PaneNode["id"]), focusedPane?.widget ?? null, dispatch);
  useEmbedRequests(!embedded, placement.openForeignWidget);

  // Hand the shell a way to arm targeting when a session is clicked in the
  // sidebar. A callback rather than a context: the sidebar is not inside this
  // tree, and one function is a smaller contract than a provider. In an effect,
  // not in render — publishing during render is a side effect, and under
  // StrictMode's double invocation it would run twice per commit.
  const { arm, openKindHere, openFile, openFileWhere, openBrowser } = placement;
  useEffect(() => {
    targetingBridge?.(forwarding ?? { arm, openKindHere, openFile, openFileWhere, openBrowser });
    // Withdrawn on unmount, so the shell's callers fall back rather than
    // dispatching into a reducer that is no longer on screen — which is what
    // the board switching the whole workspace out does.
    return () => targetingBridge?.(null);
  }, [arm, forwarding, openBrowser, openFile, openFileWhere, openKindHere, targetingBridge]);
}
