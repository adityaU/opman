import React, { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  AlignHorizontalDistributeCenter,
  AppWindow,
  Columns2,
  CopyX,
  Maximize2,
  Rows2,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

/**
 * The pane header's overflow menu.
 *
 * Every entry runs a command that also has a chord, so this is a second route
 * rather than the only one — which is what lets the header be switched off in
 * zen mode without taking anything with it. Arrow-navigable, and it returns
 * focus to whatever opened it on close, so the keyboard never gets stranded.
 */

/**
 * Icons and sections live here rather than on `PaneMenuItem`, because the
 * caller builds the list from the reducer's vocabulary and should not have to
 * carry presentation with it. Two ids drawn from the pane header's own set
 * (`Columns2`, `Rows2`, `X`) on purpose: the header button and the menu row
 * that run the same command should not look like two different commands.
 */
const ROW_ICON: Readonly<Record<string, LucideIcon>> = {
  "split-right": Columns2,
  "split-down": Rows2,
  zoom: Maximize2,
  equalize: AlignHorizontalDistributeCenter,
  "to-window": AppWindow,
  only: CopyX,
  close: X,
};

/** Make one pane two → resize what exists → move it elsewhere → end it. */
const SECTIONS: readonly (readonly string[])[] = [
  ["split-right", "split-down"],
  ["zoom", "equalize"],
  ["to-window"],
  ["only", "close"],
];

/**
 * Anything the caller adds that this file has not been taught about still gets
 * rendered — in its original order, in a trailing section — so a new action can
 * never silently vanish from the menu because its id was not listed above.
 */
function groupItems(items: readonly PaneMenuItem[]): PaneMenuItem[][] {
  const known = new Set(SECTIONS.flat());
  const groups = SECTIONS.map((ids) => items.filter((item) => ids.includes(item.id)));
  groups.push(items.filter((item) => !known.has(item.id)));
  return groups.filter((group) => group.length > 0);
}

export interface PaneMenuItem {
  readonly id: string;
  readonly label: string;
  readonly shortcut?: string;
  readonly danger?: boolean;
  readonly disabled?: boolean;
  readonly run: () => void;
}

interface PaneMenuProps {
  readonly items: readonly PaneMenuItem[];
  readonly anchor: HTMLElement;
  readonly onClose: () => void;
}

export const PaneMenu: React.FC<PaneMenuProps> = function PaneMenu({ items, anchor, onClose }) {
  const ref = useRef<HTMLDivElement>(null);
  const groups = useMemo(() => groupItems(items), [items]);
  // Arrow order is taken from the grouped order, not from the raw list, so the
  // cursor can never walk the rows in a sequence the eye does not see.
  const enabled = useMemo(() => groups.flat().filter((item) => !item.disabled), [groups]);
  const [cursor, setCursor] = useState(0);
  const box = anchor.getBoundingClientRect();

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    return () => previous?.focus?.();
  }, []);

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  }, [onClose]);

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "ArrowDown") setCursor((c) => (c + 1) % enabled.length);
    else if (event.key === "ArrowUp") setCursor((c) => (c - 1 + enabled.length) % enabled.length);
    else if (event.key === "Home") setCursor(0);
    else if (event.key === "End") setCursor(enabled.length - 1);
    else if (event.key === "Enter" || event.key === " ") {
      enabled[cursor]?.run();
      onClose();
    } else if (event.key === "Escape") onClose();
    else return;

    event.preventDefault();
    event.stopPropagation();
  };

  return createPortal(
    <div
      ref={ref}
      tabIndex={-1}
      role="menu"
      aria-label="Pane actions"
      className="modal-popover-surface wsp-menu"
      // Anchored below-right of the button, clamped so a pane at the right
      // edge of the screen does not push the menu off it. The number tracks
      // `.wsp-menu`'s min-width — the sections widened the surface.
      style={{ top: box.bottom + 4, left: Math.min(box.left, window.innerWidth - 242) }}
      onKeyDown={onKeyDown}
    >
      {groups.map((group) => (
        // Grouped rather than separated by a divider row: a `role="separator"`
        // inside a menu is one more thing for a screen reader to announce, and
        // the boundary is already carried by the group.
        <div key={group[0].id} className="wsp-menu-group" role="group">
          {group.map((item) => {
            const index = enabled.indexOf(item);
            const Icon = ROW_ICON[item.id];
            return (
              <button
                key={item.id}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                className={
                  "wsp-menu-row" +
                  (index >= 0 && index === cursor ? " is-cursor" : "") +
                  (item.danger ? " is-danger" : "")
                }
                onMouseEnter={() => index >= 0 && setCursor(index)}
                onClick={() => {
                  item.run();
                  onClose();
                }}
              >
                <span className="wsp-menu-icon" aria-hidden="true">
                  {Icon && <Icon size={13} />}
                </span>
                <span className="wsp-menu-label">{item.label}</span>
                {item.shortcut && <kbd className="wsp-menu-kbd">{item.shortcut}</kbd>}
              </button>
            );
          })}
        </div>
      ))}
    </div>,
    document.body,
  );
};
