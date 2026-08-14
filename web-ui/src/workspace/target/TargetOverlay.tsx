import React, { useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useChordLabeller } from "../../keybindings/useChord";
import { DropSlot } from "./DropSlot";
import { WindowDropStrip, type WindowDropTarget } from "./WindowDropStrip";
import type { DropEdge } from "../move";
import type { PaneId, WindowId } from "../types";
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
      /** The pane being dragged; it cannot receive itself. */
      readonly source: PaneId;
      /** Where in the target pane it landed — an edge moves it, the middle swaps. */
      readonly onDrop: (pane: PaneId, edge: DropEdge) => void;
      /** The other windows it can be sent to, drawn down the rail's edge. */
      readonly windows: readonly WindowDropTarget[];
      readonly onDropWindow: (target: WindowId | "new") => void;
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
      {rects.map(({ slot, ...rect }) =>
        interaction.kind === "drop" ? (
          <DropSlot
            key={slot.paneId}
            slot={slot}
            rect={rect}
            source={interaction.source === slot.paneId}
            onDrop={interaction.onDrop}
          />
        ) : (
          <button
            key={slot.paneId}
            type="button"
            className={"wsp-target-slot" + (slot.focused ? " is-focused" : "")}
            style={rect}
            onClick={(event) => {
              event.stopPropagation();
              interaction.onPick(slot.paneId);
            }}
            aria-label={`Open in pane ${slot.ordinal}`}
          >
            <span className="wsp-target-number">{slot.ordinal}</span>
          </button>
        ),
      )}

      {interaction.kind === "drop" && (
        <WindowDropStrip windows={interaction.windows} onDrop={interaction.onDropWindow} />
      )}

      <div className="wsp-target-chip" role="status">
        <span className="wsp-target-chip-label">{label}</span>
        <span className="wsp-target-chip-keys">
          {interaction.kind === "pick" ? <PickKeys slots={slots} /> : (
            <>edge moves · middle swaps · <kbd>esc</kbd> cancels</>
          )}
        </span>
      </div>
    </div>,
    document.body,
  );
};

/**
 * The keys, read from the keymap rather than written here.
 *
 * The overlay used to name `s`/`v`/`n` in prose, which went stale the moment
 * those bindings moved — and it never told a vim-mode user that their split key
 * is a different one. Every cap below is whatever the live keymap resolves.
 */
function PickKeys({ slots }: { readonly slots: readonly TargetSlot[] }) {
  const chordFor = useChordLabeller();
  const ordinals = slots.map((slot) => slot.ordinal).sort((left, right) => left - right);
  const first = ordinals[0];
  const last = ordinals[ordinals.length - 1];
  const paneKeys = ordinals.length > 2
    ? [String(first), "–", String(last)]
    : ordinals.map(String);

  const entries: readonly (readonly [string | undefined, string])[] = [
    [chordFor("workspace.targetSplitRight"), "vertical"],
    [chordFor("workspace.targetSplitDown"), "horizontal"],
    [chordFor("workspace.targetNewWindow"), "window"],
  ];

  return (
    <>
      {paneKeys.map((key, index) => (
        key === "–" ? <span key="dash">–</span> : <kbd key={`${key}-${index}`}>{key}</kbd>
      ))}
      {" pane · "}
      <kbd>↵</kbd> current
      {entries.filter(([chord]) => chord).map(([chord, name]) => (
        <span key={name}> · <kbd>{chord}</kbd> {name}</span>
      ))}
      {" · "}<kbd>esc</kbd>
    </>
  );
}

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
