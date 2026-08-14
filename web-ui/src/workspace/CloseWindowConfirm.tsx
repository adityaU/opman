import React, { useCallback, useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { FALLBACK_ORIGIN, useRailAnchor } from "./railAnchor";
import type { WindowId } from "./types";

/**
 * Confirming that a window should go.
 *
 * Closing a window takes every pane in it with it — terminals, editors and
 * agent transcripts that are not coming back — so the × asks first. It is
 * portalled and anchored beside the chip for the same reason the rename field
 * is: the rail is too narrow to hold a sentence and two buttons.
 *
 * Focus lands on Keep, not Close: the dangerous half of a confirmation should
 * never be one stray Enter away.
 */

interface CloseWindowConfirmProps {
  readonly windowId: WindowId;
  readonly name: string;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}

export const CloseWindowConfirm: React.FC<CloseWindowConfirmProps> = function CloseWindowConfirm({
  windowId,
  name,
  onConfirm,
  onCancel,
}) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const origin = useRailAnchor(windowId, surfaceRef);

  useLayoutEffect(() => keepRef.current?.focus(), []);

  // Escape is bound globally; a dismissed confirmation should not also close
  // whatever overlay is behind it.
  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onCancel();
    },
    [onCancel],
  );

  // Losing focus out of the popover dismisses it, but moving between its own
  // two buttons must not — hence the containment check.
  const onBlur = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
      if (event.currentTarget.contains(event.relatedTarget)) return;
      onCancel();
    },
    [onCancel],
  );

  return createPortal(
    <div
      ref={surfaceRef}
      className="wsp-confirm"
      style={origin ?? { top: FALLBACK_ORIGIN.top, left: FALLBACK_ORIGIN.left, visibility: "hidden" }}
      role="dialog"
      aria-label="Close window"
      onKeyDown={onKeyDown}
      onBlur={onBlur}
    >
      <span className="wsp-confirm-text">
        Close <strong>{name}</strong>? Everything open in it goes with it.
      </span>
      <span className="wsp-confirm-actions">
        <button type="button" className="wsp-confirm-btn is-danger" onClick={onConfirm}>
          Close
        </button>
        <button ref={keepRef} type="button" className="wsp-confirm-btn" onClick={onCancel}>
          Keep
        </button>
      </span>
    </div>,
    document.body,
  );
};
