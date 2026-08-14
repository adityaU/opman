import React, { useCallback, useEffect, useMemo, useState } from "react";
import { PaneMenu } from "./PaneMenu";
import { RenameWindowField } from "./RenameWindowField";
import { WorkspaceRoot, type PaneContext } from "./WorkspaceRoot";
import { WindowSwitcher } from "./WindowSwitcher";
import { TargetOverlay } from "./target/TargetOverlay";
import { useTargeting, type TargetRequest, type TargetSlot } from "./target/useTargeting";
import { WidgetOpener } from "./opener/WidgetOpener";
import { usePaneMenu } from "./usePaneMenu";
import { useWhenContext } from "../keybindings/useCommand";
import { useWorkspaceCommands } from "./useWorkspaceCommands";
import { useWorkspacePlacement } from "./useWorkspacePlacement";
import { useWorkspaceWidgets } from "./useWorkspaceWidgets";
import { useWorkspace } from "./useWorkspace";
import { useTerminalActivity } from "./useTerminalActivity";
import { useHeaderPeek } from "./useHeaderPeek";
import { panes } from "./tree";
import type { PaneId, WidgetKind, WidgetState, WindowId } from "./types";
import { WorkspaceChatProvider, type WorkspaceChatServices } from "./widgets/WorkspaceChatContext";

/**
 * The desktop workspace, assembled.
 *
 * Owns the transient state the reducer has no business holding — which overlay
 * is up, which pane's menu is open — and nothing else. Everything durable lives
 * in the reducer and is persisted; everything transient dies with a reload,
 * which is the correct behaviour for a menu.
 *
 * Note how little of this file changes on a window switch. `activateWindow`
 * moves one field of the reducer state, and every prop reaching `WorkspaceRoot`
 * below is either that field or something built to hold its identity across it.
 * That is deliberate — see `WindowView`.
 */

export interface WorkspaceBridge {
  readonly arm: (request: TargetRequest) => void;
  readonly openKindHere: (kind: WidgetKind) => void;
  /** Reveal a file: in the files pane already open, or in a new split. */
  readonly openFile: (path: string, line: number | null) => void;
  /** Reveal a file in a pane the user picks, through the target overlay. */
  readonly openFileWhere: (path: string, line: number | null, label: string) => void;
  /**
   * Reveal the project's browser, which an agent has just driven somewhere:
   * reusing the pane already showing it, or splitting a column for it.
   */
  readonly openBrowser: (projectPath: string, url: string) => void;
}

export interface WorkspaceProject {
  readonly path: string;
  readonly name: string;
}

export interface DesktopWorkspaceProps {
  readonly projects: readonly WorkspaceProject[];
  /** Sessions of a project, for the opener's third step. */
  readonly sessionsFor: (projectPath: string) => readonly { id: string; title: string; updated: number }[];
  readonly describe: (widget: WidgetState | null) => PaneContext;
  readonly busySessions: ReadonlySet<string>;
  readonly onError: (message: string) => void;
  readonly activeSessionId: string | null;
  /**
   * App-level chat services. `bindSession` is supplied here rather than by the
   * shell: only the workspace can write a lazily-created session back onto the
   * pane that created it.
   */
  readonly chat: Omit<WorkspaceChatServices, "bindSession" | "setEngine">;
  /**
   * Publishes the actions the shell needs to drive from outside the tree — a
   * sidebar session click, and the surviving `layout.toggle*` chords. One
   * registrant per command id: two would race on mount order.
   */
  readonly targetingBridge?: (api: WorkspaceBridge | null) => void;
}

export const DesktopWorkspace: React.FC<DesktopWorkspaceProps> = function DesktopWorkspace({
  projects,
  sessionsFor,
  describe,
  busySessions,
  onError,
  activeSessionId,
  chat,
  targetingBridge,
}) {
  const { state, window: activeWindow, dispatch } = useWorkspace(true);
  const targeting = useTargeting();

  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [renaming, setRenaming] = useState<WindowId | null>(null);

  const paneList = useMemo(() => panes(activeWindow.root), [activeWindow.root]);

  const menu = usePaneMenu({ panes: paneList, dispatch, describe });

  // The panel keybindings — terminal tabs, git actions, editor navigation — are
  // each scoped to the surface they act on. That surface is whichever pane has
  // focus, so this is the only place that can answer for them.
  const focusedKind = useMemo(
    () => paneList.find((pane) => pane.id === activeWindow.focusedPaneId)?.widget?.kind ?? null,
    [activeWindow.focusedPaneId, paneList],
  );
  useWhenContext({
    editorOpen: focusedKind === "files",
    terminalOpen: focusedKind === "terminal",
    gitRepo: focusedKind === "git",
  });

  const { chatServices, renderWidget } = useWorkspaceWidgets({ state, dispatch, chat, onError });

  const placement = useWorkspacePlacement({
    projects,
    sessionsFor,
    dispatch,
    targeting,
    root: activeWindow.root,
    focusedPaneId: activeWindow.focusedPaneId,
    panes: paneList,
    // The same cache the menu reads, so the opener's shell step and a Recent row
    // pointing at a shell cannot name it differently.
    shells: menu.shells,
  });

  // A terminal pane is busy when a command owns its terminal. Chat panes get
  // theirs from the session's own busy state; files and git have no long-running
  // background work of their own — every operation they run is a request that
  // shows its own progress and is over before a pulse would register.
  const busyTerminals = useTerminalActivity(state.windows);

  const busyWindows = useMemo(() => {
    const busy = new Set<WindowId>();
    for (const window of state.windows) {
      const anyBusy = panes(window.root).some((pane) => {
        if (busyTerminals.has(pane.id)) return true;
        if (pane.widget?.kind !== "chat" || !pane.widget.sessionId) return false;
        return busySessions.has(pane.widget.sessionId);
      });
      if (anyBusy) busy.add(window.id);
    }
    return busy;
  }, [busySessions, busyTerminals, state.windows]);

  // Hand the shell a way to arm targeting when a session is clicked in the
  // sidebar. A callback rather than a context: the sidebar is not inside this
  // tree, and one function is a smaller contract than a provider. In an effect,
  // not in render — publishing during render is a side effect, and under
  // StrictMode's double invocation it would run twice per commit.
  const { armOrPlace, openKindHere, openFileHere, openFileWhere, openBrowserHere } = placement;
  useEffect(() => {
    targetingBridge?.({
      arm: armOrPlace,
      openKindHere,
      openFile: openFileHere,
      openFileWhere,
      openBrowser: openBrowserHere,
    });
    // Withdrawn on unmount, so the shell's callers fall back rather than
    // dispatching into a reducer that is no longer on screen — which is what
    // the board switching the whole workspace out does.
    return () => targetingBridge?.(null);
  }, [armOrPlace, openBrowserHere, openFileHere, openFileWhere, openKindHere, targetingBridge]);

  /** Open the inline field. The commit goes through `commitRename`. */
  const renameWindow = useCallback(
    (id: WindowId) => setRenaming(state.windows.some((w) => w.id === id) ? id : null),
    [state.windows],
  );

  const commitRename = useCallback(
    (name: string) => {
      if (renaming) dispatch({ type: "renameWindow", window: renaming, name });
      setRenaming(null);
    },
    [dispatch, renaming],
  );

  const headerPeek = useHeaderPeek();

  const renamingWindow = renaming
    ? state.windows.find((candidate) => candidate.id === renaming) ?? null
    : null;

  // ── Commands ──

  useWorkspaceCommands({
    workspace: state,
    focusedPaneId: activeWindow.focusedPaneId,
    dispatch,
    openWidgetPicker: placement.openWidgetPicker,
    openWindowSwitcher: () => setSwitcherOpen(true),
    openPaneMenu: () => {
      // The corner button, not a header one: it is the control that is always
      // on screen, so the menu opens in the same place whichever route is used.
      const element = document.querySelector<HTMLElement>(
        `[data-pane-id="${CSS.escape(activeWindow.focusedPaneId)}"] .wsp-pane-dots`,
      );
      if (element) menu.show(activeWindow.focusedPaneId, element);
    },
    peekPaneHeader: headerPeek.peek,
    renameActiveWindow: () => renameWindow(activeWindow.id),
    targeting: targeting.active,
    openerOpen: placement.opener !== null,
    resolveTarget: placement.resolveTarget,
    resolveTargetByOrdinal: placement.resolveTargetByOrdinal,
    resolveTargetSplit: placement.resolveTargetSplit,
    resolveTargetNewWindow: placement.resolveTargetNewWindow,
    cancelTargeting: targeting.cancel,
  });

  // ── Render ──

  /**
   * The windows a dragged pane can be sent to: every one but the one it is in,
   * which is what the pane slots themselves already answer for.
   */
  const otherWindows = useMemo(
    () =>
      state.windows
        .filter((window) => window.id !== state.activeWindowId)
        .map((window) => ({ id: window.id, name: window.name })),
    [state.activeWindowId, state.windows],
  );

  const slots: TargetSlot[] = useMemo(
    () =>
      paneList.map((pane, index) => ({
        paneId: pane.id,
        ordinal: index + 1,
        focused: pane.id === activeWindow.focusedPaneId,
      })),
    [activeWindow.focusedPaneId, paneList],
  );

  return (
    <WorkspaceChatProvider value={chatServices}>
      <WorkspaceRoot
        workspace={state}
        dispatch={dispatch}
        peekHeaders={headerPeek.peeking}
        describePane={describe}
        busyPanes={busyTerminals}
        busyWindows={busyWindows}
        onOpenWidget={placement.onOpenWidgetKind}
        onPaneMenu={menu.show}
        onRenameWindow={renameWindow}
        dragSourcePane={placement.dragSource}
        onDragWidget={placement.setDragSource}
        onDragWidgetEnd={placement.endDrag}
        renderWidget={renderWidget}
      />

      {targeting.request && (
        <TargetOverlay
          label={targeting.request.label}
          slots={slots}
          interaction={{ kind: "pick", onPick: placement.resolveTarget }}
          onCancel={targeting.cancel}
        />
      )}

      {placement.dragSource && paneList.length > 1 && (
        <TargetOverlay
          label={placement.draggedLabel}
          slots={slots}
          interaction={{
            kind: "drop",
            source: placement.dragSource,
            onDrop: placement.dropWidget,
            windows: otherWindows,
            onDropWindow: placement.dropOnWindow,
          }}
          onCancel={placement.endDrag}
        />
      )}

      {placement.opener && (
        <WidgetOpener
          choicesFor={placement.choicesFor}
          initialDraft={placement.opener.draft}
          onDone={placement.onOpenerDone}
          onCancel={placement.closeOpener}
        />
      )}

      {switcherOpen && (
        <WindowSwitcher
          windows={state.windows}
          activeWindowId={state.activeWindowId}
          busyWindows={busyWindows}
          onPick={(id) => {
            dispatch({ type: "activateWindow", window: id });
            setSwitcherOpen(false);
          }}
          onCancel={() => setSwitcherOpen(false)}
        />
      )}

      {menu.open && (
        <PaneMenu items={menu.items} anchor={menu.open.anchor} onClose={menu.close} />
      )}

      {renamingWindow && (
        <RenameWindowField
          windowId={renamingWindow.id}
          name={renamingWindow.name}
          onCommit={commitRename}
          onCancel={() => setRenaming(null)}
        />
      )}
    </WorkspaceChatProvider>
  );
};
