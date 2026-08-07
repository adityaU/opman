import React from "react";
import { FileCode2, GitBranch, MessageSquare, SquareTerminal } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { PaneId, WidgetKind } from "./types";

/**
 * What a freshly split pane shows.
 *
 * A blank rectangle would make splitting feel like a two-step action, so the
 * pane offers the four widgets directly. It is the same choice the staged
 * opener starts with — this is that first step, inlined where the user is
 * already looking, rather than a second way of doing it. The rows are built
 * to the opener's own recipe so that reads as one mechanism, and so that a
 * screen of three empty panes stays quiet instead of repeating a card grid.
 */

const CHOICES: readonly { kind: WidgetKind; label: string; icon: LucideIcon; hint: string }[] = [
  { kind: "chat", label: "Chat", icon: MessageSquare, hint: "a session" },
  { kind: "files", label: "Files", icon: FileCode2, hint: "editor + explorer" },
  { kind: "terminal", label: "Terminal", icon: SquareTerminal, hint: "a shell" },
  { kind: "git", label: "Git", icon: GitBranch, hint: "changes + log" },
];

interface EmptyPaneProps {
  readonly paneId: PaneId;
  readonly compact: boolean;
  readonly onChoose: (pane: PaneId, kind: WidgetKind) => void;
}

export const EmptyPane: React.FC<EmptyPaneProps> = React.memo(function EmptyPane({
  paneId,
  compact,
  onChoose,
}) {
  return (
    <div className={`wsp-empty${compact ? " is-compact" : ""}`}>
      {/* One block, centred as a unit, left-aligned inside — so the label, the
          rows and the shortcut line all start on the same edge. */}
      <div className="wsp-empty-block">
        {!compact && <p className="wsp-empty-label">Open in this pane</p>}
        <div className="wsp-empty-list" role="group" aria-label="Choose what to open in this pane">
          {CHOICES.map(({ kind, label, icon: Icon, hint }) => (
            <button
              key={kind}
              type="button"
              className="wsp-empty-choice"
              onClick={() => onChoose(paneId, kind)}
            >
              <Icon size={compact ? 13 : 15} />
              <span className="wsp-empty-choice-label">{label}</span>
              {!compact && <span className="wsp-empty-choice-hint">{hint}</span>}
            </button>
          ))}
        </div>
        {!compact && (
          <p className="wsp-empty-hint">
            <kbd>⌘K</kbd> <kbd>O</kbd> opens the full picker
          </p>
        )}
      </div>
    </div>
  );
});
