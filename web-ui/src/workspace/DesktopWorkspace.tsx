import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { PaneMenu, type PaneMenuItem } from "./PaneMenu";
import { RenameWindowField } from "./RenameWindowField";
import { WorkspaceRoot, type PaneContext } from "./WorkspaceRoot";
import { WindowSwitcher } from "./WindowSwitcher";
import { PaneWidget } from "./widgets/PaneWidget";
import { TargetOverlay } from "./target/TargetOverlay";
import { useTargeting, type TargetRequest, type TargetSlot } from "./target/useTargeting";
import { WidgetOpener, type OpenerChoice } from "./opener/WidgetOpener";
import { formatTime } from "../sidebar/formatTime";
import { EMPTY_DRAFT, type StepId, type OpenerDraft } from "./opener/steps";
import { useWorkspaceCommands } from "./useWorkspaceCommands";
import { useWorkspace } from "./useWorkspace";
import { useTerminalActivity } from "./useTerminalActivity";
import { panes } from "./tree";
import { paneByOrdinal } from "./nav";
import { WIDGET_KINDS, type PaneEngine, type PaneId, type WidgetKind, type WidgetState, type WindowId } from "./types";
import { findPane } from "./tree";
import { WorkspaceChatProvider, type WorkspaceChatServices } from "./widgets/WorkspaceChatContext";

/**
 * The desktop workspace, assembled.
 *
 * Owns the transient state the reducer has no business holding — which overlay
 * is up, which pane's menu is open — and nothing else. Everything durable lives
 * in the reducer and is persisted; everything transient dies with a reload,
 * which is the correct behaviour for a menu.
 */

export interface WorkspaceBridge {
  readonly arm: (request: TargetRequest) => void;
  readonly openKindHere: (kind: WidgetKind) => void;
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
  readonly targetingBridge?: (api: WorkspaceBridge) => void;
}

export const DesktopWorkspace: React.FC<DesktopWorkspaceProps> = function DesktopWorkspace({
  projects,
  sessionsFor,
  describe,
  busySessions,
  onError,
  chat,
  targetingBridge,
}) {
  const { state, window: activeWindow, dispatch } = useWorkspace(true);
  const targeting = useTargeting();

  const [opener, setOpener] = useState<{ draft: OpenerDraft } | null>(null);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [menu, setMenu] = useState<{ pane: PaneId; anchor: HTMLElement } | null>(null);
  const [renaming, setRenaming] = useState<WindowId | null>(null);
  // Kept apart from `targeting`: the targeting commands (1-9, s/v, n) are gated
  // on it, and a pointer drag must not arm a keymap the user cannot see.
  const [dragSource, setDragSource] = useState<PaneId | null>(null);

  // Hand the shell a way to arm targeting when a session is clicked in the
  // sidebar. A callback rather than a context: the sidebar is not inside this
  // tree, and one function is a smaller contract than a provider. In an effect,
  // not in render — publishing during render is a side effect, and under
  // StrictMode's double invocation it would run twice per commit.
  const openKindHere = useCallback(
    (kind: WidgetKind) => onOpenWidgetKindRef.current(kind),
    [],
  );
  const onOpenWidgetKindRef = useRef<(kind: WidgetKind) => void>(() => {});

  const paneList = useMemo(() => panes(activeWindow.root), [activeWindow.root]);

  /**
   * A chat pane opened as "new session" has no id until its first send creates
   * one. Writing it back here is what makes the pane survive a reload as that
   * conversation rather than as another blank composer.
   */
  const bindSession = useCallback(
    (paneId: string, sessionId: string) => {
      const pane = findPane(activeWindow.root, paneId as PaneId);
      if (!pane || pane.widget?.kind !== "chat" || pane.widget.sessionId) return;
      dispatch({
        type: "setWidget",
        pane: pane.id,
        widget: { ...pane.widget, sessionId },
      });
    },
    [activeWindow.root, dispatch],
  );

  /**
   * Persist a terminal pane's live PTY ids so its shells survive a reload.
   * Guarded on an actual change: the panel reports on every tab render, and an
   * unguarded dispatch would rewrite the tree — and re-save it — every frame.
   */
  const onPtyIdsChanged = useCallback(
    (paneId: PaneId, ptyIds: readonly string[]) => {
      const pane = findPane(activeWindow.root, paneId);
      if (!pane || pane.widget?.kind !== "terminal") return;
      const current = pane.widget.ptyIds;
      if (current.length === ptyIds.length && current.every((id, i) => id === ptyIds[i])) return;
      dispatch({ type: "setWidget", pane: paneId, widget: { ...pane.widget, ptyIds } });
    },
    [activeWindow.root, dispatch],
  );

  /**
   * Give a chat pane an engine of its own. Persisted with the layout, so two
   * panes on two runners survive a reload as two panes on two runners.
   */
  const setEngine = useCallback(
    (paneId: string, engine: PaneEngine) => {
      const pane = findPane(activeWindow.root, paneId as PaneId);
      if (!pane || pane.widget?.kind !== "chat") return;
      dispatch({ type: "setWidget", pane: pane.id, widget: { ...pane.widget, engine } });
    },
    [activeWindow.root, dispatch],
  );

  const chatServices = useMemo<WorkspaceChatServices>(
    () => ({ ...chat, bindSession, setEngine }),
    [bindSession, chat, setEngine],
  );

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

  // ── Placing a widget ──

  const place = useCallback(
    (widget: WidgetState, pane: PaneId) => dispatch({ type: "setWidget", pane, widget }),
    [dispatch],
  );

  const resolveTarget = useCallback(
    (pane: PaneId) => {
      const request = targeting.take();
      if (request) place(request.widget, pane);
    },
    [place, targeting],
  );

  const resolveTargetByOrdinal = useCallback(
    (ordinal: number) => {
      const pane = paneByOrdinal(activeWindow.root, ordinal);
      if (pane) resolveTarget(pane);
    },
    [activeWindow.root, resolveTarget],
  );

  const resolveTargetSplit = useCallback(
    (dir: "row" | "col") => {
      const request = targeting.take();
      if (request) {
        dispatch({ type: "splitPane", pane: activeWindow.focusedPaneId, dir, widget: request.widget });
      }
    },
    [activeWindow.focusedPaneId, dispatch, targeting],
  );

  const resolveTargetNewWindow = useCallback(() => {
    const request = targeting.take();
    if (request) dispatch({ type: "newWindow", widget: request.widget });
  }, [dispatch, targeting]);

  /**
   * The inline four-button opener on an empty pane. It answers step one, so
   * the project still has to be chosen — unless there is only one, in which
   * case asking would be ceremony.
   */
  const onOpenWidgetKind = useCallback(
    (pane: PaneId, kind: WidgetKind) => {
      if (projects.length === 1) {
        place(widgetFor(kind, projects[0].path), pane);
        return;
      }
      dispatch({ type: "focusPane", pane });
      // Carry the kind through: the four buttons *are* step one.
      setOpener({ draft: { ...EMPTY_DRAFT, kind } });
    },
    [dispatch, place, projects],
  );

  onOpenWidgetKindRef.current = (kind: WidgetKind) =>
    onOpenWidgetKind(activeWindow.focusedPaneId, kind);

  // ── The staged opener ──

  const choicesFor = useCallback(
    (step: StepId, draft: OpenerDraft): readonly OpenerChoice[] => {
      if (step === "kind") {
        return WIDGET_KINDS.map((kind) => ({ value: kind, label: KIND_LABEL[kind] }));
      }
      if (step === "project") {
        return projects.map((project) => ({
          value: project.path,
          label: project.name,
          hint: project.path,
        }));
      }
      // Recency-sorted by `sessionsFor`; "New session" is pinned above it
      // because it is the answer to a different question than "which one".
      const sessions = draft.projectPath ? sessionsFor(draft.projectPath) : [];
      return [
        { value: null, label: "New session", hint: "created on first send" },
        ...sessions.map((session) => ({
          value: session.id,
          label: session.title,
          hint: formatTime(session.updated),
        })),
      ];
    },
    [projects, sessionsFor],
  );

  /**
   * Ask which pane — unless the question has only one answer.
   *
   * With a single pane the overlay was a whole extra gesture between the user
   * and the thing they clicked: it dimmed the screen to offer one slot, marked
   * "1", which they then had to hit. Asking before replacing is worth a click
   * when there is somewhere else the widget could have gone; when there is
   * not, it is just a click.
   */
  const armOrPlace = useCallback(
    (request: TargetRequest) => {
      if (paneList.length === 1) {
        place(request.widget, paneList[0].id);
        return;
      }
      targeting.arm(request);
    },
    [paneList, place, targeting],
  );

  useEffect(() => {
    targetingBridge?.({ arm: armOrPlace, openKindHere });
  }, [armOrPlace, openKindHere, targetingBridge]);

  const onOpenerDone = useCallback(
    (widget: WidgetState) => {
      setOpener(null);
      // Straight into the focused pane when it is empty; otherwise ask where,
      // rather than silently replacing something the user is looking at.
      const focused = paneList.find((pane) => pane.id === activeWindow.focusedPaneId);
      if (focused && !focused.widget) place(widget, focused.id);
      else armOrPlace({ widget, label: describeWidget(widget, projects) });
    },
    [activeWindow.focusedPaneId, armOrPlace, paneList, place, projects],
  );

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

  const dropWidget = useCallback(
    (pane: PaneId) => {
      if (dragSource) dispatch({ type: "swapWidgets", from: dragSource, to: pane });
      setDragSource(null);
    },
    [dispatch, dragSource],
  );

  const draggedLabel = useMemo(() => {
    if (!dragSource) return "";
    const pane = findPane(activeWindow.root, dragSource);
    return pane?.widget ? describeWidget(pane.widget, projects) : "";
  }, [activeWindow.root, dragSource, projects]);

  const renamingWindow = renaming
    ? state.windows.find((candidate) => candidate.id === renaming) ?? null
    : null;

  // ── Commands ──

  useWorkspaceCommands({
    workspace: state,
    focusedPaneId: activeWindow.focusedPaneId,
    dispatch,
    openWidgetPicker: () => setOpener({ draft: EMPTY_DRAFT }),
    openWindowSwitcher: () => setSwitcherOpen(true),
    openPaneMenu: () => {
      const element = document.querySelector<HTMLElement>(
        `[data-pane-id="${CSS.escape(activeWindow.focusedPaneId)}"] .wsp-head-btn`,
      );
      if (element) setMenu({ pane: activeWindow.focusedPaneId, anchor: element });
    },
    renameActiveWindow: () => renameWindow(activeWindow.id),
    targeting: targeting.active,
    openerOpen: opener !== null,
    resolveTarget,
    resolveTargetByOrdinal,
    resolveTargetSplit,
    resolveTargetNewWindow,
    cancelTargeting: targeting.cancel,
  });

  // ── Render ──

  const slots: TargetSlot[] = useMemo(
    () =>
      paneList.map((pane, index) => ({
        paneId: pane.id,
        ordinal: index + 1,
        focused: pane.id === activeWindow.focusedPaneId,
      })),
    [activeWindow.focusedPaneId, paneList],
  );

  const menuItems = useMemo<PaneMenuItem[]>(
    () => (menu ? buildMenu(menu.pane, dispatch, paneList.length) : []),
    [dispatch, menu, paneList.length],
  );

  return (
    <WorkspaceChatProvider value={chatServices}>
      <WorkspaceRoot
        workspace={state}
        dispatch={dispatch}
        describePane={describe}
        busyPanes={busyTerminals}
        busyWindows={busyWindows}
        onOpenWidget={onOpenWidgetKind}
        onPaneMenu={(pane, anchor) => setMenu({ pane, anchor })}
        onRenameWindow={renameWindow}
        dragSourcePane={dragSource}
        onDragWidget={setDragSource}
        onDragWidgetEnd={() => setDragSource(null)}
        renderWidget={(widget, pane) => (
          <PaneWidget
            widget={widget}
            pane={pane}
            focused={pane.id === activeWindow.focusedPaneId}
            onError={onError}
            onPtyIdsChanged={onPtyIdsChanged}
          />
        )}
      />

      {targeting.request && (
        <TargetOverlay
          label={targeting.request.label}
          slots={slots}
          interaction={{ kind: "pick", onPick: resolveTarget }}
          onCancel={targeting.cancel}
        />
      )}

      {dragSource && paneList.length > 1 && (
        <TargetOverlay
          label={draggedLabel}
          slots={slots}
          interaction={{ kind: "drop", source: dragSource, onDrop: dropWidget }}
          onCancel={() => setDragSource(null)}
        />
      )}

      {opener && (
        <WidgetOpener
          choicesFor={choicesFor}
          initialDraft={opener.draft}
          onDone={onOpenerDone}
          onCancel={() => setOpener(null)}
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

      {menu && (
        <PaneMenu items={menuItems} anchor={menu.anchor} onClose={() => setMenu(null)} />
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

// ── Helpers ─────────────────────────────────────────────

const KIND_LABEL: Readonly<Record<WidgetKind, string>> = {
  chat: "Chat",
  files: "Files",
  terminal: "Terminal",
  git: "Git",
};

function widgetFor(kind: WidgetKind, projectPath: string): WidgetState {
  switch (kind) {
    case "chat":
      return { kind: "chat", projectPath, sessionId: null , engine: null};
    case "files":
      return { kind: "files", projectPath, filePath: null };
    case "terminal":
      return { kind: "terminal", projectPath, ptyIds: [] };
    case "git":
      return { kind: "git", projectPath };
  }
}

function describeWidget(widget: WidgetState, projects: readonly WorkspaceProject[]): string {
  const project = projects.find((candidate) => candidate.path === widget.projectPath);
  return `${KIND_LABEL[widget.kind]} · ${project?.name ?? widget.projectPath}`;
}

function buildMenu(
  pane: PaneId,
  dispatch: (action: import("./reducer").WorkspaceAction) => void,
  total: number,
): PaneMenuItem[] {
  return [
    { id: "split-right", label: "Split right", shortcut: "⌘\\", run: () => dispatch({ type: "splitPane", pane, dir: "row" }) },
    { id: "split-down", label: "Split down", shortcut: "⌘K ⌘\\", run: () => dispatch({ type: "splitPane", pane, dir: "col" }) },
    { id: "zoom", label: "Zoom", shortcut: "⌘K Z", disabled: total < 2, run: () => dispatch({ type: "toggleZoom" }) },
    { id: "equalize", label: "Reset sizes", shortcut: "⌘K =", run: () => dispatch({ type: "equalize" }) },
    { id: "to-window", label: "Move to new window", disabled: total < 2, run: () => dispatch({ type: "movePaneToWindow", pane, window: "new" }) },
    { id: "only", label: "Close other panes", shortcut: "⌘K U", disabled: total < 2, run: () => dispatch({ type: "closeOthers", pane }) },
    { id: "close", label: "Close pane", shortcut: "⌘K Q", danger: true, disabled: total < 2, run: () => dispatch({ type: "closePane", pane }) },
  ];
}
