import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  AlignHorizontalDistributeCenter,
  AppWindow,
  ArrowLeft,
  ArrowRight,
  Columns2,
  CopyX,
  History,
  Maximize2,
  Rows2,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { CommandId } from "../keybindings/types";
import { useChordLabeller } from "../keybindings/useChord";

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
  back: ArrowLeft,
  forward: ArrowRight,
  "split-right": Columns2,
  "split-down": Rows2,
  zoom: Maximize2,
  equalize: AlignHorizontalDistributeCenter,
  "to-window": AppWindow,
  only: CopyX,
  close: X,
};

/**
 * Where the pane has been → make one pane two → resize what exists → move it
 * elsewhere → end it → where else it has been.
 *
 * Recent rows are matched by prefix rather than listed, because there is one per
 * entry in the pane's trail and their ids carry the index to jump to.
 */
const RECENT_PREFIX = "recent:";

const SECTIONS: readonly (readonly string[])[] = [
  ["back", "forward"],
  ["split-right", "split-down"],
  ["zoom", "equalize"],
  ["to-window"],
  ["only", "close"],
];

/**
 * Anything the caller adds that this file has not been taught about still gets
 * rendered — in its original order, in a trailing section — so a new action can
 * never silently vanish from the menu because its id was not listed above. The
 * recent rows land there deliberately rather than by accident, which is why they
 * are the last thing `paneMenuItems` returns.
 */
function groupItems(items: readonly PaneMenuItem[]): PaneMenuItem[][] {
  const known = new Set(SECTIONS.flat());
  const groups = SECTIONS.map((ids) => items.filter((item) => ids.includes(item.id)));
  groups.push(items.filter((item) => !known.has(item.id)));
  return groups.filter((group) => group.length > 0);
}

/** History rows are named by what they are, not by an icon per row. */
function iconFor(id: string): LucideIcon | undefined {
  return id.startsWith(RECENT_PREFIX) ? History : ROW_ICON[id];
}

export interface PaneMenuItem {
  readonly id: string;
  readonly label: string;
  /** The command this row runs. Its live chord is shown beside the label. */
  readonly command?: CommandId;
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
  const chordFor = useChordLabeller();
  const groups = useMemo(() => groupItems(items), [items]);
  // Arrow order is taken from the grouped order, not from the raw list, so the
  // cursor can never walk the rows in a sequence the eye does not see.
  const enabled = useMemo(() => groups.flat().filter((item) => !item.disabled), [groups]);
  const [cursor, setCursor] = useState(0);

  /**
   * Anchored below-right of the button, then nudged back on screen.
   *
   * Measured rather than assumed. This used to clamp against a hard-coded number
   * tracking `.wsp-menu`'s min-width, which was already a guess and became a
   * wrong one as soon as the menu grew a history section: a pane at the right
   * edge showing "Back to some-long-name.ts" ran off the viewport, and twelve
   * rows on a short window ran off the bottom. Flipping above the button when
   * there is no room below is what a menu at the bottom of the screen needs.
   */
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const button = anchor.getBoundingClientRect();
    const menu = node.getBoundingClientRect();
    const margin = 8;

    const left = Math.max(margin, Math.min(button.left, window.innerWidth - menu.width - margin));
    const below = button.bottom + 4;
    const fitsBelow = below + menu.height + margin <= window.innerHeight;
    const top = fitsBelow
      ? below
      : Math.max(margin, Math.min(button.top - menu.height - 4, window.innerHeight - menu.height - margin));

    node.style.left = `${left}px`;
    node.style.top = `${top}px`;
  }, [anchor, items]);

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
      // Placed off screen for one frame so it can be measured before it is
      // seen; the layout effect above puts it where it belongs.
      style={{ top: -9999, left: -9999 }}
      onKeyDown={onKeyDown}
    >
      {groups.map((group) => (
        // Grouped rather than separated by a divider row: a `role="separator"`
        // inside a menu is one more thing for a screen reader to announce, and
        // the boundary is already carried by the group.
        <div key={group[0].id} className="wsp-menu-group" role="group">
          {group.map((item) => {
            const index = enabled.indexOf(item);
            const Icon = iconFor(item.id);
            const chord = chordFor(item.command);
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
                {chord && <kbd className="wsp-menu-kbd">{chord}</kbd>}
              </button>
            );
          })}
        </div>
      ))}
    </div>,
    document.body,
  );
};
