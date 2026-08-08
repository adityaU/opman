/**
 * ExplorerContextMenu — the one place a row's actions live.
 *
 * The tree used to sprout five icon buttons on hover, on top of the filename
 * they were meant to act on. That trades the panel's actual job — reading
 * names — for controls used once a session. Actions now open on right-click
 * (or the row's trailing button), portalled to the body so no ancestor's
 * overflow can clip them, and labelled in words rather than guessed from a
 * 12px glyph.
 */
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CommandId } from "../../keybindings/types";
import { useChordLabeller } from "../../keybindings/useChord";

export interface MenuAction {
  key: string;
  label: string;
  icon: React.ReactNode;
  /** The command this row runs; its live chord is shown against the label. */
  command?: CommandId;
  danger?: boolean;
  run: () => void;
}

interface Props {
  x: number;
  y: number;
  title: string;
  actions: MenuAction[];
  onClose: () => void;
}

const MARGIN = 8;

export function ExplorerContextMenu({ x, y, title, actions, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const chordFor = useChordLabeller();
  const [pos, setPos] = useState({ left: x, top: y });
  const [active, setActive] = useState(0);

  // Flip the menu back inside the viewport before the first paint, so it never
  // appears offscreen and jumps.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const left = Math.max(MARGIN, Math.min(x, window.innerWidth - width - MARGIN));
    const top = y + height + MARGIN > window.innerHeight ? Math.max(MARGIN, y - height) : y;
    setPos({ left, top });
  }, [x, y]);

  useEffect(() => {
    ref.current?.focus();
    const dismiss = (event: Event) => {
      if (ref.current?.contains(event.target as Node)) return;
      onClose();
    };
    // The contextmenu event that opened this menu is still propagating toward
    // the document, so binding synchronously would dismiss it on the same
    // click. Bind on the next frame instead.
    const armed = requestAnimationFrame(() => {
      document.addEventListener("mousedown", dismiss);
      document.addEventListener("contextmenu", dismiss);
    });
    window.addEventListener("resize", onClose);
    window.addEventListener("blur", onClose);
    return () => {
      cancelAnimationFrame(armed);
      document.removeEventListener("mousedown", dismiss);
      document.removeEventListener("contextmenu", dismiss);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") { event.preventDefault(); onClose(); return; }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((i) => (i + 1) % actions.length);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((i) => (i - 1 + actions.length) % actions.length);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const action = actions[active];
      if (action) { action.run(); onClose(); }
    }
  };

  return createPortal(
    <div
      ref={ref}
      className="xpl-ctx modal-popover-surface"
      role="menu"
      tabIndex={-1}
      aria-label={`Actions for ${title}`}
      style={{ left: pos.left, top: pos.top }}
      onKeyDown={onKeyDown}
    >
      <div className="xpl-ctx-title" title={title}>{title}</div>
      {actions.map((action, index) => {
        const chord = chordFor(action.command);
        return (
          <button
            key={action.key}
            type="button"
            role="menuitem"
            className={`xpl-ctx-item${action.danger ? " is-danger" : ""}${index === active ? " is-active" : ""}`}
            onMouseEnter={() => setActive(index)}
            onClick={() => { action.run(); onClose(); }}
          >
            {action.icon}
            <span className="xpl-ctx-label">{action.label}</span>
            {chord && <kbd className="xpl-ctx-hint">{chord}</kbd>}
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
