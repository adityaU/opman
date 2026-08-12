import React, { Suspense, lazy } from "react";
import type { PaneId, PaneNode, WidgetState } from "../types";

/**
 * Renders a pane's widget.
 *
 * Existing widgets drop straight in — the editor, git and terminal panels were
 * already parameterised by `projectPath`, and chat owns its session. Neovim is
 * likewise pane-local: its session comes from the widget rather than the
 * globally focused chat session.
 *
 * Lazy, matching how the old side panel loaded these: a workspace restored with
 * four panes should not block first paint on four bundles.
 */

const CodeEditorPanel = lazy(() => import("../../code-editor"));
const GitPanel = lazy(() => import("../../git-panel"));
const TerminalPanel = lazy(() =>
  import("../../TerminalPanel").then((m) => ({ default: m.TerminalPanel })),
);
const ChatWidget = lazy(() => import("./ChatWidget").then((m) => ({ default: m.ChatWidget })));

export interface PaneWidgetProps {
  readonly widget: WidgetState;
  readonly pane: PaneNode;
  readonly focused: boolean;
  readonly onError: (message: string) => void;
  /** Persist the pane's live PTY ids so its terminals survive a reload. */
  readonly onPtyIdsChanged: (pane: PaneId, ptyIds: readonly string[]) => void;
}

export const PaneWidget: React.FC<PaneWidgetProps> = React.memo(function PaneWidget({
  widget,
  pane,
  focused,
  onError,
  onPtyIdsChanged,
}) {
  return (
    <Suspense fallback={<div className="wsp-widget-loading" aria-busy="true" />}>
      {renderWidget(widget, pane, focused, onError, onPtyIdsChanged)}
    </Suspense>
  );
});

function renderWidget(
  widget: WidgetState,
  pane: PaneNode,
  focused: boolean,
  onError: (message: string) => void,
  onPtyIdsChanged: (pane: PaneId, ptyIds: readonly string[]) => void,
): React.ReactNode {
  switch (widget.kind) {
    case "chat":
      return (
        <ChatWidget
          paneId={pane.id}
          projectPath={widget.projectPath}
          sessionId={widget.sessionId}
          engine={widget.engine}
          focused={focused}
        />
      );

    case "files":
      return (
        <CodeEditorPanel
          layout="desktop"
          focused={focused}
          projectPath={widget.projectPath}
          sessionId={widget.sessionId}
          open={widget.open}
          onError={onError}
        />
      );

    case "terminal":
      // `visible` is always true: a pane that is not being shown is not
      // mounted, so there is no hidden-then-revealed case to re-fit for.
      return (
        <TerminalPanel
          layout="desktop"
          sessionId={null}
          projectPath={widget.projectPath}
          visible
          restoreIds={widget.ptyIds}
          onTabsChanged={(ids) => onPtyIdsChanged(pane.id, ids)}
          onClose={() => {}}
        />
      );

    case "git":
      return <GitPanel focused={focused} projectPath={widget.projectPath} onError={onError} />;
  }
}
