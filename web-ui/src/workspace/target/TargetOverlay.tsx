import React, { useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { PaneId } from "../types";
import type { TargetSlot } from "./useTargeting";

/**
 * The numbers painted over every pane while a pane is being chosen.
 *
 * Rendered as a portal positioned from live pane rects rather than inside each
 * pane: a pane's widget may be a scrolled transcript or an xterm canvas, and
 * putting an absolutely-positioned child in there would either be clipped by
 * its overflow or force it to establish a stacking context it does not want.
 *
 * Two ways in, one surface. Arming a widget answers "where does this go?" with
 * a keystroke; dragging a pane header answers it with a pointer. They resolve
 * to the same rects on purpose — a second drop-target visual would be a second
 * thing to learn for the same question.
 *
 * The keys are not handled here. They are real commands gated on
 * `workspaceTargeting`, so they show up in the keybindings view and can be
 * rebound like anything else — an overlay is not a place the keymap stops.
 */

/** How the pane is chosen. The two modes never share an affordance. */
export type TargetInteraction =
  | { readonly kind: "pick"; readonly onPick: (pane: PaneId) => void }
  | {
      readonly kind: "drop";
      /** The pane the widget is being dragged out of; it cannot receive itself. */
      readonly source: PaneId;
      readonly onDrop: (pane: PaneId) => void;
    };

interface TargetOverlayProps {
  /** What is being placed, e.g. a session title. */
  readonly label: string;
  readonly slots: readonly TargetSlot[];
  readonly interaction: TargetInteraction;
  readonly onCancel: () => void;
}

interface Rect {
  readonly slot: TargetSlot;
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

export const TargetOverlay: React.FC<TargetOverlayProps> = function TargetOverlay({
  label,
  slots,
  interaction,
  onCancel,
}) {
  const rects = usePaneRects(slots);
  const [over, setOver] = useState<PaneId | null>(null);

  return createPortal(
    <div
      className="wsp-target"
      role="dialog"
      aria-label="Choose a pane"
      onClick={onCancel}
      // A drop that lands on the backdrop rather than a slot is a cancel, not a
      // navigation — without this the browser would treat it as a link drop.
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        onCancel();
      }}
    >
      {rects.map(({ slot, top, left, width, height }) => {
        const source = interaction.kind === "drop" && interaction.source === slot.paneId;
        return (
          <button
            key={slot.paneId}
            type="button"
            className={
              "wsp-target-slot" +
              (slot.focused ? " is-focused" : "") +
              (source ? " is-source" : "") +
              (over === slot.paneId ? " is-over" : "")
            }
            style={{ top, left, width, height }}
            onClick={(event) => {
              event.stopPropagation();
              if (interaction.kind === "pick") interaction.onPick(slot.paneId);
            }}
            onDragOver={(event) => {
              if (source) return;
              // preventDefault is what marks the element as a drop target.
              event.preventDefault();
              setOver(slot.paneId);
            }}
            onDragLeave={() => setOver((current) => (current === slot.paneId ? null : current))}
            onDrop={(event) => {
              event.preventDefault();
              event.stopPropagation();
              setOver(null);
              if (interaction.kind === "drop" && !source) interaction.onDrop(slot.paneId);
              else onCancel();
            }}
            aria-label={`Open in pane ${slot.ordinal}`}
          >
            <span className="wsp-target-number">{slot.ordinal}</span>
          </button>
        );
      })}

      <div className="wsp-target-chip" role="status">
        <span className="wsp-target-chip-label">{label}</span>
        <span className="wsp-target-chip-keys">
          {interaction.kind === "pick" ? (
            <>
              <kbd>1</kbd>–<kbd>9</kbd> pane · <kbd>↵</kbd> current · <kbd>s</kbd>/<kbd>v</kbd> split ·{" "}
              <kbd>n</kbd> window · <kbd>esc</kbd>
            </>
          ) : (
            <>drop on a pane to swap · <kbd>esc</kbd> cancels</>
          )}
        </span>
      </div>
    </div>,
    document.body,
  );
};

/**
 * Measure the panes once per arm.
 *
 * A ResizeObserver would be wrong here: the overlay is transient and blocks
 * pointer interaction with the tree underneath, so nothing can resize while it
 * is up — except the window, which is worth one listener.
 */
function usePaneRects(slots: readonly TargetSlot[]): Rect[] {
  const [rects, setRects] = useState<Rect[]>([]);

  useLayoutEffect(() => {
    const measure = () => {
      const measured = slots.flatMap((slot) => {
        const element = document.querySelector(`[data-pane-id="${CSS.escape(slot.paneId)}"]`);
        if (!element) return [];
        const box = element.getBoundingClientRect();
        return [{ slot, top: box.top, left: box.left, width: box.width, height: box.height }];
      });
      setRects(measured);
    };
    measure();

    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [slots]);

  return rects;
}
