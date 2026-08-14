import React, { useCallback } from "react";
import {
  Columns2,
  FileCode2,
  GitBranch,
  Globe,
  Maximize2,
  MessageSquare,
  Minimize2,
  Rows2,
  SquareTerminal,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { KeyHint } from "../keybindings/hint/KeyHint";
import { ProjectBadge } from "./ProjectBadge";
import type { PaneId, WidgetKind, WidgetState } from "./types";

/**
 * The toolbar over a pane, on the same recipe as every other panel header in
 * the app — but out of the layout, and out of sight until asked for.
 *
 * Everything here is also a command, which is what lets the header be absent
 * by default without taking any capability with it: the reveal chord brings it
 * back for a few seconds, and the pane menu is permanently in the corner. What
 * identifies a pane the rest of the time is what the card itself draws — the
 * project hue in its border, and the busy rim.
 */

const WIDGET_ICON: Readonly<Record<WidgetKind, LucideIcon>> = {
  chat: MessageSquare,
  files: FileCode2,
  terminal: SquareTerminal,
  git: GitBranch,
  browser: Globe,
};

const WIDGET_LABEL: Readonly<Record<WidgetKind, string>> = {
  chat: "Chat",
  files: "Files",
  terminal: "Terminal",
  git: "Git",
  browser: "Browser",
};

export interface PaneHeaderProps {
  readonly paneId: PaneId;
  readonly ordinal: number;
  readonly widget: WidgetState | null;
  readonly projectName: string;
  /** Session title, file name, terminal tab or branch — whatever is live. */
  readonly subtitle: string | null;
  readonly busy: boolean;
  readonly focused: boolean;
  /** Visible: the reveal chord is up, or the header itself holds focus. */
  readonly peek: boolean;
  readonly canClose: boolean;
  readonly onSplit: (pane: PaneId, dir: "row" | "col") => void;
  readonly onClose: (pane: PaneId) => void;
  /** Zen: this pane alone, filling the shell. */
  readonly zen: boolean;
  readonly onToggleZen: () => void;
  /** Begin dragging this pane's widget to another pane. */
  readonly onDragWidget: (pane: PaneId) => void;
  readonly onDragWidgetEnd: () => void;
}

export const PaneHeader: React.FC<PaneHeaderProps> = React.memo(function PaneHeader({
  paneId,
  ordinal,
  widget,
  projectName,
  subtitle,
  busy,
  focused,
  peek,
  canClose,
  onSplit,
  onClose,
  zen,
  onToggleZen,
  onDragWidget,
  onDragWidgetEnd,
}) {
  const Icon = widget ? WIDGET_ICON[widget.kind] : null;

  /**
   * The kind chip is the grab handle — the one part of the header that names
   * what would move. `dataTransfer` is set because Firefox refuses to start a
   * drag without it; the pane id travels in React state, not in the payload,
   * so a drag from another window or app can never be mistaken for one of ours.
   */
  const startDrag = useCallback(
    (event: React.DragEvent<HTMLSpanElement>) => {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", paneId);
      onDragWidget(paneId);
    },
    [onDragWidget, paneId],
  );

  return (
    <div className={`wsp-head${focused ? " is-focused" : ""}${peek ? " is-peek" : ""}`}>
      <span className="wsp-head-ordinal" aria-hidden="true">
        {ordinal}
      </span>

      {widget ? (
        <>
          <ProjectBadge projectPath={widget.projectPath} name={projectName} busy={busy} />
          <span className="wsp-head-sep" aria-hidden="true" />
          <span
            className="wsp-head-widget"
            draggable
            onDragStart={startDrag}
            onDragEnd={onDragWidgetEnd}
            title={`Drag to move this ${WIDGET_LABEL[widget.kind].toLowerCase()} to another pane`}
          >
            {Icon && <Icon size={13} />}
            <span className="wsp-head-kind">{WIDGET_LABEL[widget.kind]}</span>
          </span>
          {subtitle && <span className="wsp-head-subtitle">{subtitle}</span>}
        </>
      ) : (
        <span className="wsp-head-empty">Empty pane</span>
      )}

      <span className="wsp-head-actions">
        <KeyHint label="Split right" command="workspace.splitRight">
          <button
            type="button"
            className="wsp-head-btn"
            onClick={() => onSplit(paneId, "row")}
            aria-label="Split pane right"
          >
            <Columns2 size={13} />
          </button>
        </KeyHint>
        <KeyHint label="Split down" command="workspace.splitDown">
          <button
            type="button"
            className="wsp-head-btn"
            onClick={() => onSplit(paneId, "col")}
            aria-label="Split pane down"
          >
            <Rows2 size={13} />
          </button>
        </KeyHint>
        {/* Leaving Zen is Escape, which is a mode exit rather than a binding of
            its own, so it is the one chord here written out by hand. */}
        <KeyHint
          label={zen ? "Leave Zen" : "Zen — fill the screen"}
          command={zen ? undefined : "workspace.toggleZen"}
          chord={zen ? "Esc" : undefined}
        >
          <button
            type="button"
            className={`wsp-head-btn wsp-head-btn-zen${zen ? " is-on" : ""}`}
            onClick={onToggleZen}
            aria-pressed={zen}
            aria-label={zen ? "Leave Zen" : "Zen: fill the screen with this pane"}
          >
            {zen ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
          </button>
        </KeyHint>
        {canClose && (
          <KeyHint label="Close pane" command="workspace.closePane">
            <button
              type="button"
              className="wsp-head-btn wsp-head-btn-close"
              onClick={() => onClose(paneId)}
              aria-label="Close pane"
            >
              <X size={13} />
            </button>
          </KeyHint>
        )}
      </span>
    </div>
  );
});

export { WIDGET_ICON, WIDGET_LABEL };
