import React, { useCallback, useEffect, useRef } from "react";
import { PaneHeader } from "./PaneHeader";
import { projectColorVars } from "./ProjectBadge";
import type { PaneId, PaneNode, WidgetState } from "./types";

/**
 * One pane: a panel card with an optional header and a widget inside it.
 *
 * The project's colour is carried by the card's own border (workspace-2.css
 * mixes `--pane-hue` into it), which is why the pane still sets the hue as an
 * inline custom property even though it draws no coloured element itself.
 *
 * The pane is a focus scope, not just a box. `workspace.focusPaneN` moves real
 * DOM focus in here, which is what makes every widget's existing keymap work
 * per-pane with no changes: `useSurfaceFocus` reads `data-surface` off the
 * nearest ancestor, so focusing pane 3's git panel scopes every `focus==git`
 * binding to *that* panel.
 */

export interface PaneProps {
  readonly pane: PaneNode;
  readonly ordinal: number;
  readonly focused: boolean;
  readonly showHeader: boolean;
  readonly canClose: boolean;
  readonly projectName: string;
  readonly subtitle: string | null;
  readonly busy: boolean;
  readonly onFocus: (pane: PaneId) => void;
  readonly onSplit: (pane: PaneId, dir: "row" | "col") => void;
  readonly onClose: (pane: PaneId) => void;
  readonly onMenu: (pane: PaneId, anchor: HTMLElement) => void;
  readonly zen: boolean;
  readonly onToggleZen: () => void;
  readonly onDragWidget: (pane: PaneId) => void;
  readonly onDragWidgetEnd: () => void;
  /** True while this pane's own widget is the one being dragged. */
  readonly dragSource: boolean;
  readonly children: React.ReactNode;
}

export const Pane: React.FC<PaneProps> = React.memo(function Pane({
  pane,
  ordinal,
  focused,
  showHeader,
  canClose,
  projectName,
  subtitle,
  busy,
  onFocus,
  onSplit,
  onClose,
  onMenu,
  zen,
  onToggleZen,
  onDragWidget,
  onDragWidgetEnd,
  dragSource,
  children,
}) {
  const ref = useRef<HTMLDivElement>(null);
  useAdoptFocus(ref, focused);

  // Pointer-down rather than click: pressing anywhere in a pane makes it the
  // one the next command acts on, even when the press lands on dead space.
  const claim = useCallback(() => onFocus(pane.id), [onFocus, pane.id]);

  return (
    <section
      ref={ref}
      tabIndex={-1}
      data-pane-id={pane.id}
      aria-label={paneLabel(pane.widget, projectName, ordinal)}
      className={
        `wsp-pane${focused ? " is-focused" : ""}${busy ? " is-busy" : ""}` +
        (dragSource ? " is-drag-source" : "")
      }
      style={pane.widget ? projectColorVars(pane.widget.projectPath) : undefined}
      onPointerDownCapture={claim}
      onFocusCapture={claim}
    >
      {showHeader && (
        <PaneHeader
          paneId={pane.id}
          ordinal={ordinal}
          widget={pane.widget}
          projectName={projectName}
          subtitle={subtitle}
          busy={busy}
          focused={focused}
          canClose={canClose}
          onSplit={onSplit}
          onClose={onClose}
          onMenu={onMenu}
          zen={zen}
          onToggleZen={onToggleZen}
          onDragWidget={onDragWidget}
          onDragWidgetEnd={onDragWidgetEnd}
        />
      )}
      <div className="wsp-pane-body">{children}</div>
    </section>
  );
});

function paneLabel(widget: WidgetState | null, projectName: string, ordinal: number): string {
  if (!widget) return `Pane ${ordinal}, empty`;
  return `Pane ${ordinal}, ${widget.kind} in ${projectName}`;
}

/**
 * Pull DOM focus into the pane when it becomes the focused one — but only if
 * focus is not already inside it, and only if its window is on screen.
 *
 * Without the first guard, a pane containing a text input would steal the caret
 * back to its own wrapper on every unrelated re-render, and typing into a
 * composer would drop characters.
 *
 * The second guard is what lets `focused` mean "this window's focused pane"
 * rather than "the focused pane". Every mounted window has one, and only the
 * visible window's may hold the caret; `WindowLayer` adopts it on the way in.
 */
function useAdoptFocus(ref: React.RefObject<HTMLElement>, focused: boolean): void {
  useEffect(() => {
    if (!focused) return;
    const element = ref.current;
    if (!element) return;
    if (element.closest(".wsp-window:not(.is-active)")) return;
    if (element.contains(document.activeElement)) return;
    element.focus({ preventScroll: true });
  }, [focused, ref]);
}
