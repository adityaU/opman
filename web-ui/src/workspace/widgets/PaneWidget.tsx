import React, { Suspense, lazy } from "react";
import type { PaneId, PaneNode, WidgetState } from "../types";

/**
 * Renders a pane's widget.
 *
 * Existing widgets drop straight in — the editor, git and terminal panels were
 * already parameterised by `projectPath`, and chat owns its session. The
 * editor's language servers are likewise pane-local: their session comes from
 * the widget rather than from the globally focused chat session.
 *
 * Lazy, matching how the old side panel loaded these: a workspace restored with
 * four panes should not block first paint on four bundles.
 */

const CodeEditorPanel = lazy(() => import("../../code-editor"));
const GitPanel = lazy(() => import("../../git"));
const TerminalPanel = lazy(() =>
  import("../../TerminalPanel").then((m) => ({ default: m.TerminalPanel })),
);
const ChatWidget = lazy(() => import("./ChatWidget").then((m) => ({ default: m.ChatWidget })));
const BrowserPanel = lazy(() => import("../../browser-panel"));

export interface PaneWidgetProps {
  readonly widget: WidgetState;
  readonly pane: PaneNode;
  readonly focused: boolean;
  readonly onError: (message: string) => void;
  /** Persist which shell the pane's terminal is showing, so a reload comes
   *  back to it rather than to the picker. */
  readonly onPtyIdChanged: (pane: PaneId, ptyId: string | null) => void;
  /** Persist a browser pane's current URL so a reload comes back to it. */
  readonly onBrowserUrlChanged: (pane: PaneId, url: string) => void;
  /**
   * Report which file a files pane is on, so the pane's history knows where it
   * has been. The panel's own cursor, not the request it was handed — clicking
   * around the tree is how a pane usually reaches a file.
   */
  readonly onActiveFileChanged: (pane: PaneId, path: string | null) => void;
}

export const PaneWidget: React.FC<PaneWidgetProps> = React.memo(function PaneWidget(props) {
  return (
    <Suspense fallback={<div className="wsp-widget-loading" aria-busy="true" />}>
      {renderWidget(props)}
    </Suspense>
  );
});

/**
 * Takes the props whole rather than as positional arguments. There are five
 * write-backs now, all of the same `(pane, value)` shape, and a positional list
 * of five same-shaped callbacks is a list two of them can be swapped in without
 * the compiler noticing.
 */
function renderWidget({
  widget,
  pane,
  focused,
  onError,
  onPtyIdChanged,
  onBrowserUrlChanged,
  onActiveFileChanged,
}: PaneWidgetProps): React.ReactNode {
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
          onActiveFileChanged={(path) => onActiveFileChanged(pane.id, path)}
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
          ptyId={widget.ptyId}
          onPtyIdChanged={(ptyId) => onPtyIdChanged(pane.id, ptyId)}
        />
      );

    case "git":
      return <GitPanel focused={focused} projectPath={widget.projectPath} onError={onError} />;

    case "browser":
      return (
        <BrowserPanel
          paneId={widget.browserId}
          project={widget.projectPath}
          url={widget.url}
          reveal={widget.reveal}
          focused={focused}
          onUrlChanged={(url) => onBrowserUrlChanged(pane.id, url)}
        />
      );
  }
}
