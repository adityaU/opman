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

/** Where the field sits when the rail is collapsed and there is no chip. */
const FALLBACK_ORIGIN = { top: 12, left: 12 } as const;

export const RenameWindowField: React.FC<RenameWindowFieldProps> = function RenameWindowField({
  windowId,
  name,
  onCommit,
  onCancel,
}) {
  const [value, setValue] = useState(name);
  const origin = useAnchor(windowId);
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
    <div className="wsp-rename" style={origin} role="dialog" aria-label="Rename window">
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

/** Position beside the window's rail chip, or at the fallback origin. */
function useAnchor(windowId: WindowId): { top: number; left: number } {
  const [origin, setOrigin] = useState<{ top: number; left: number }>(FALLBACK_ORIGIN);

  useLayoutEffect(() => {
    const chip = document.querySelector(`[data-window-id="${CSS.escape(windowId)}"]`);
    if (!chip) return;
    const box = chip.getBoundingClientRect();
    // Centred on the chip rather than aligned to its top, so the field reads as
    // the chip opened up rather than as something dropped next to it.
    setOrigin({ top: box.top + box.height / 2, left: box.right + 6 });
  }, [windowId]);

  return origin;
}
