/**
 * The panel's one confirmation surface.
 *
 * It portals to `document.body` because the git panel scrolls inside a pane
 * with `overflow` ancestors, which would clip a dialog rendered in place. It
 * adopts the app's shared modal contract by name — `gitp-confirm` plus the
 * documented `-header` / `-body` / `-footer` parts — rather than inventing a
 * second surface with its own radius and shadow.
 */

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle } from "lucide-react";

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: React.ReactNode;
  confirmLabel: string;
  danger?: boolean;
  /** When set, the confirm button stays disabled until this is typed exactly. */
  requireTyped?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  danger,
  requireTyped,
  onConfirm,
  onCancel,
}: ConfirmDialogProps): JSX.Element | null {
  const titleId = useId();
  const surface = useRef<HTMLDivElement | null>(null);
  const restoreTo = useRef<HTMLElement | null>(null);
  const [typed, setTyped] = useState("");

  // Remember the trigger before focus moves, so closing returns the caret to
  // the row action the person actually pressed.
  useEffect(() => {
    if (!open) return;
    restoreTo.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setTyped("");
    const first = surface.current?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? surface.current)?.focus();
    return () => {
      restoreTo.current?.focus();
    };
  }, [open]);

  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onCancel();
        return;
      }
      if (event.key !== "Tab") return;
      const nodes = Array.from(surface.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []);
      if (nodes.length === 0) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || active === surface.current)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    },
    [onCancel],
  );

  if (!open) return null;

  const blocked = requireTyped !== undefined && typed !== requireTyped;

  return createPortal(
    <div className="gitp-confirm-backdrop" onMouseDown={onCancel}>
      <div
        ref={surface}
        className="gitp-confirm"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <div className="gitp-confirm-header">
          {danger ? <AlertTriangle size={16} className="gitp-confirm-icon" aria-hidden="true" /> : null}
          <h2 className="gitp-confirm-title" id={titleId}>
            {title}
          </h2>
        </div>

        <div className="gitp-confirm-body">
          {body}
          {requireTyped !== undefined ? (
            <label className="gitp-confirm-typed">
              <span className="gitp-confirm-typed-label">
                Type <code className="gitp-mono">{requireTyped}</code> to confirm
              </span>
              <input
                className="gitp-input"
                value={typed}
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => setTyped(event.target.value)}
              />
            </label>
          ) : null}
        </div>

        <div className="gitp-confirm-footer">
          <button type="button" className="gitp-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className={danger ? "gitp-btn gitp-btn-danger" : "gitp-btn gitp-btn-primary"}
            disabled={blocked}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
