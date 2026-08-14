import React, { useCallback, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { WindowId } from "./types";

/**
 * Renaming a window, in place.
 *
 * Portalled and anchored to the rail chip rather than rendered inside it: the
 * rail is 44px wide with `overflow-x: hidden`, so a field big enough to type a
 * name into would be clipped by its own container. Anchoring keeps the edit
 * visually attached to the chip it belongs to; portalling lets it be legible.
 *
 * The rail lives on the right edge of the shell, so the field opens to the
 * chip's *left* whenever the space on its right cannot hold it — placing it
 * blindly to the right put the input off-screen and made renaming impossible.
 * Both axes are clamped to the viewport for the same reason.
 *
 * It also has to work when the rail is collapsed to the spine, where there is
 * no chip to point at — hence the fallback position rather than a null render.
 * A command that silently does nothing in one chrome mode is a broken command.
 */

interface RenameWindowFieldProps {
  readonly windowId: WindowId;
  readonly name: string;
  readonly onCommit: (name: string) => void;
  readonly onCancel: () => void;
}

/** Gap between the chip and the field, on whichever side it opens. */
const GUTTER = 6;
/** Keep-out margin so a clamped field never touches the viewport edge. */
const EDGE = 8;

interface Origin {
  readonly top: number;
  readonly left: number;
}

/** Where the field sits when the rail is collapsed and there is no chip. */
const FALLBACK_ORIGIN: Origin = { top: 48, left: EDGE };

export const RenameWindowField: React.FC<RenameWindowFieldProps> = function RenameWindowField({
  windowId,
  name,
  onCommit,
  onCancel,
}) {
  const [value, setValue] = useState(name);
  const fieldRef = useRef<HTMLDivElement>(null);
  const origin = useAnchor(windowId, fieldRef);
  const inputRef = useRef<HTMLInputElement>(null);

  useLayoutEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    input.select();
  }, []);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      // Stopped here rather than left to bubble: Escape and Enter are both
      // bound globally, and a window rename should not also close an overlay.
      if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        onCommit(value);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCancel();
      }
    },
    [onCancel, onCommit, value],
  );

  return createPortal(
    <div
      ref={fieldRef}
      className="wsp-rename"
      // Hidden for the one frame before it is measured and placed, so the field
      // is never painted at the pre-clamp position.
      style={origin ?? { top: FALLBACK_ORIGIN.top, left: FALLBACK_ORIGIN.left, visibility: "hidden" }}
      role="dialog"
      aria-label="Rename window"
    >
      <input
        ref={inputRef}
        className="wsp-rename-input"
        type="text"
        value={value}
        maxLength={24}
        aria-label="Window name"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={onKeyDown}
        onBlur={onCancel}
      />
    </div>,
    document.body,
  );
};

/**
 * Position beside the window's rail chip, or at the fallback origin.
 *
 * Returns null until the field has been measured — the caller keeps it hidden
 * for that frame.
 */
function useAnchor(
  windowId: WindowId,
  fieldRef: React.RefObject<HTMLDivElement | null>,
): Origin | null {
  const [origin, setOrigin] = useState<Origin | null>(null);

  useLayoutEffect(() => {
    const field = fieldRef.current;
    if (!field) return;
    const chip = document.querySelector(`[data-window-id="${CSS.escape(windowId)}"]`);
    if (!chip) {
      setOrigin(FALLBACK_ORIGIN);
      return;
    }
    setOrigin(place(chip.getBoundingClientRect(), field.getBoundingClientRect()));
  }, [fieldRef, windowId]);

  return origin;
}

/**
 * Open to the chip's right when that side fits, otherwise to its left — the
 * rail sits on the right edge, so the flip is the common case, not the corner
 * one. `top` is the field's centre: the CSS lifts it by half its own height.
 */
function place(chip: DOMRect, field: DOMRect): Origin {
  const right = chip.right + GUTTER;
  const fitsRight = right + field.width <= window.innerWidth - EDGE;
  const left = fitsRight ? right : chip.left - GUTTER - field.width;
  const half = field.height / 2;
  return {
    top: clamp(chip.top + chip.height / 2, EDGE + half, window.innerHeight - EDGE - half),
    left: clamp(left, EDGE, Math.max(EDGE, window.innerWidth - EDGE - field.width)),
  };
}

function clamp(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}
