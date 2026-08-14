import React, { useCallback, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { FALLBACK_ORIGIN, useRailAnchor } from "./railAnchor";
import type { WindowId } from "./types";

/**
 * Renaming a window, in place.
 *
 * Portalled and anchored to the rail chip rather than rendered inside it: the
 * rail is 44px wide with `overflow-x: hidden`, so a field big enough to type a
 * name into would be clipped by its own container. Anchoring keeps the edit
 * visually attached to the chip it belongs to; portalling lets it be legible.
 *
 * Placement — the flip to the chip's left, the viewport clamp, the collapsed
 * rail's fallback — is `useRailAnchor`, shared with the close confirmation.
 */

interface RenameWindowFieldProps {
  readonly windowId: WindowId;
  readonly name: string;
  readonly onCommit: (name: string) => void;
  readonly onCancel: () => void;
}

export const RenameWindowField: React.FC<RenameWindowFieldProps> = function RenameWindowField({
  windowId,
  name,
  onCommit,
  onCancel,
}) {
  const [value, setValue] = useState(name);
  const fieldRef = useRef<HTMLDivElement>(null);
  const origin = useRailAnchor(windowId, fieldRef);
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
