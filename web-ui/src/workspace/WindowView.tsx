import React, { useCallback } from "react";
import { EmptyPane } from "./EmptyPane";
import { Pane } from "./Pane";
import { PaneTree } from "./PaneTree";
import { ordinalOfPane } from "./nav";
import { paneCount } from "./tree";
import type { PaneContext } from "./WorkspaceRoot";
import type {
  PaneId,
  PaneNode,
  SplitId,
  WidgetKind,
  WidgetState,
  WorkspaceWindow,
} from "./types";

/**
 * One window's pane tree, memoised on that window alone.
 *
 * This is what makes switching windows cheap. Every mounted window is a child
 * of the same root, so before this existed a change to *any* field of the
 * workspace — `activeWindowId` above all — re-rendered all of them, and each
 * one dragged a full chat transcript through react-markdown on the way. A
 * switch cost a couple of hundred milliseconds of work on trees that had not
 * changed by so much as a pixel.
 *
 * The memo only holds if every prop is stable, which is why nothing here takes
 * "is this the visible window": that is the one input that changes on a switch,
 * and it belongs to the layer wrapping this, not to the tree inside it. A pane
 * is the focused pane of its own window whether or not that window is on
 * screen — `Pane` is what declines to pull DOM focus while hidden.
 */

export interface WindowViewProps {
  readonly win: WorkspaceWindow;
  readonly describePane: (widget: WidgetState | null) => PaneContext;
  readonly renderWidget: (widget: WidgetState, pane: PaneNode, focused: boolean) => React.ReactNode;
  /** Panes busy for a reason the widget itself reports, e.g. a running command. */
  readonly busyPanes: ReadonlySet<PaneId>;
  readonly showHeaders: boolean;
  readonly zen: boolean;
  readonly dragSourcePane: PaneId | null;
  readonly onFocus: (pane: PaneId) => void;
  readonly onSplit: (pane: PaneId, dir: "row" | "col") => void;
  readonly onClose: (pane: PaneId) => void;
  readonly onMenu: (pane: PaneId, anchor: HTMLElement) => void;
  readonly onToggleZen: () => void;
  readonly onOpenWidget: (pane: PaneId, kind: WidgetKind) => void;
  readonly onDragWidget: (pane: PaneId) => void;
  readonly onDragWidgetEnd: () => void;
  readonly onResize: (split: SplitId, index: number, delta: number) => void;
  readonly onEqualize: () => void;
}

export const WindowView: React.FC<WindowViewProps> = React.memo(function WindowView(props) {
  const { win, describePane, renderWidget, busyPanes, showHeaders, zen, dragSourcePane } = props;
  const { onFocus, onSplit, onClose, onMenu, onToggleZen, onOpenWidget } = props;
  const { onDragWidget, onDragWidgetEnd, onResize, onEqualize } = props;

  const total = paneCount(win.root);

  const renderPane = useCallback(
    (pane: PaneNode) => {
      const context = describePane(pane.widget);
      const focused = pane.id === win.focusedPaneId;
      return (
        <Pane
          key={pane.id}
          pane={pane}
          ordinal={ordinalOfPane(win.root, pane.id) ?? 1}
          focused={focused}
          showHeader={showHeaders}
          canClose={total > 1}
          projectName={context.projectName}
          subtitle={context.subtitle}
          busy={context.busy || busyPanes.has(pane.id)}
          onFocus={onFocus}
          onSplit={onSplit}
          onClose={onClose}
          onMenu={onMenu}
          zen={zen}
          onToggleZen={onToggleZen}
          onDragWidget={onDragWidget}
          onDragWidgetEnd={onDragWidgetEnd}
          dragSource={pane.id === dragSourcePane}
        >
          {pane.widget ? (
            renderWidget(pane.widget, pane, focused)
          ) : (
            <EmptyPane paneId={pane.id} compact={total > 3} onChoose={onOpenWidget} />
          )}
        </Pane>
      );
    },
    [
      busyPanes,
      describePane,
      dragSourcePane,
      onClose,
      onDragWidget,
      onDragWidgetEnd,
      onFocus,
      onMenu,
      onOpenWidget,
      onSplit,
      onToggleZen,
      renderWidget,
      showHeaders,
      total,
      win.focusedPaneId,
      win.root,
      zen,
    ],
  );

  return (
    <PaneTree
      node={win.root}
      zoomedPaneId={win.zoomedPaneId}
      renderPane={renderPane}
      onResize={onResize}
      onEqualize={onEqualize}
    />
  );
});
