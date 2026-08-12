import React, { useCallback, useMemo, useRef } from "react";
import { ChatSidebar } from "../ChatSidebar";
import { SidebarPeek } from "./SidebarPeek";
import { DesktopWorkspace, type WorkspaceBridge, type WorkspaceProject } from "./DesktopWorkspace";
import type { PaneContext } from "./WorkspaceRoot";
import type { WorkspaceChatServices } from "./widgets/WorkspaceChatContext";
import type { WidgetState } from "./types";

/**
 * The desktop shell: the sidebar, and the workspace beside it.
 *
 * This is the boundary the redesign draws. Everything left of it — the sidebar
 * — is unchanged; everything right of it is panes. Mobile never reaches here,
 * so `MobileDock` and the old fixed panels are untouched by any of it.
 */

export interface DesktopShellProps {
  readonly sidebar: React.ComponentProps<typeof ChatSidebar>;
  readonly sidebarVisible: boolean;
  readonly sidebarWidth: number;
  readonly sidebarResizeHandle: React.HTMLAttributes<HTMLDivElement>;
  readonly projects: readonly WorkspaceProject[];
  readonly sessionsFor: (projectPath: string) => readonly { id: string; title: string; updated: number }[];
  readonly describe: (widget: WidgetState | null) => PaneContext;
  readonly busySessions: ReadonlySet<string>;
  readonly chat: Omit<WorkspaceChatServices, "bindSession" | "setEngine">;
  readonly onError: (message: string) => void;
  readonly activeSessionId: string | null;
  /** Receives the workspace's outward-facing actions, and null once it leaves. */
  readonly onTargetingReady: (api: WorkspaceBridge | null) => void;
}

export const DesktopShell: React.FC<DesktopShellProps> = function DesktopShell({
  sidebar,
  sidebarVisible,
  sidebarWidth,
  sidebarResizeHandle,
  projects,
  sessionsFor,
  describe,
  busySessions,
  chat,
  onError,
  activeSessionId,
  onTargetingReady,
}) {
  // Stable identity: `DesktopWorkspace` publishes through this in an effect, so
  // a new function each render would re-publish on every keystroke.
  const readyRef = useRef(onTargetingReady);
  readyRef.current = onTargetingReady;
  const targetingBridge = useCallback((api: WorkspaceBridge | null) => {
    readyRef.current(api);
  }, []);

  const sidebarNode = useMemo(() => <ChatSidebar {...sidebar} />, [sidebar]);

  return (
    <div className="chat-content" data-surface="chat">
      {sidebarVisible ? (
        <>
          <div style={{ width: sidebarWidth, flexShrink: 0 }}>{sidebarNode}</div>
          <div {...sidebarResizeHandle} />
        </>
      ) : (
        // Hidden, it peeks as a floating overlay rather than by re-entering the
        // layout — resizing the tree would refit every xterm and jump every
        // transcript's scroll on a stray mouse move against the left edge.
        <SidebarPeek width={sidebarWidth}>{sidebarNode}</SidebarPeek>
      )}

      <DesktopWorkspace
        projects={projects}
        sessionsFor={sessionsFor}
        describe={describe}
        busySessions={busySessions}
        chat={chat}
        onError={onError}
        activeSessionId={activeSessionId}
        targetingBridge={targetingBridge}
      />
    </div>
  );
};
