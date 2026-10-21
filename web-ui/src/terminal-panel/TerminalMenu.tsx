import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Clipboard, Copy, Eraser, MousePointer2, Search, TextSelect } from "lucide-react";
import type { CommandId } from "../keybindings/types";
import { useChordLabeller } from "../keybindings/useChord";
import { isMac, shortcutLabel } from "./keys";

/**
 * The terminal's right-click menu.
 *
 * Portalled to the body so a pane's overflow or transform cannot clip it, and styled with
 * the same menu classes as the sidebar's, so it follows the glassy and flat themes with no
 * rules of its own. Closes on any outside click, Escape, scroll or resize.
 */

export interface TerminalMenuProps {
  readonly x: number;
  readonly y: number;
  readonly canCopy: boolean;
  readonly onCopy: () => void;
  readonly onPaste: () => void;
  readonly onSelectAll: () => void;
  readonly onFind: () => void;
  readonly onCopyMode: () => void;
  readonly onClear: () => void;
  readonly onClose: () => void;
}

const PAD = 8;

export function TerminalMenu(props: TerminalMenuProps) {
  const { x, y, canCopy, onClose } = props;
  const ref = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState({ left: x, top: y });
  const chordFor = useChordLabeller();
  const mac = isMac();

  // Measured after layout so the menu is kept on screen whatever its height turns out to be.
  useLayoutEffect(() => {
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;
    setAt({
      left: Math.max(PAD, Math.min(x, window.innerWidth - box.width - PAD)),
      top: Math.max(PAD, Math.min(y, window.innerHeight - box.height - PAD)),
    });
    ref.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [x, y]);

  useEffect(() => {
    const outside = (event: Event) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", key, true);
    window.addEventListener("resize", onClose);
    window.addEventListener("blur", onClose);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", key, true);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const item = (
    label: string,
    icon: React.ReactNode,
    run: () => void,
    command?: CommandId,
    disabled = false,
    shortcut?: string,
  ) => {
    const chord = shortcut ?? (command ? chordFor(command) : undefined);
    return (
      <button
        type="button"
        role="menuitem"
        className="sb-context-item"
        disabled={disabled}
        onClick={() => {
          onClose();
          run();
        }}
      >
        {icon}
        {label}
        {chord && <kbd className="sb-context-kbd">{chord}</kbd>}
      </button>
    );
  };

  const move = (event: React.KeyboardEvent) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const rows = Array.from(ref.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = (at + (event.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length;
    rows[next]?.focus();
  };

  return createPortal(
    <div
      ref={ref}
      className="sb-context-menu term-menu"
      role="menu"
      aria-label="Terminal"
      style={{ left: at.left, top: at.top }}
      onKeyDown={move}
      onContextMenu={(event) => event.preventDefault()}
    >
      {item("Copy", <Copy size={12} />, props.onCopy, undefined, !canCopy, shortcutLabel("copy", mac))}
      {item("Paste", <Clipboard size={12} />, props.onPaste, undefined, false, shortcutLabel("paste", mac))}
      {item("Select All", <TextSelect size={12} />, props.onSelectAll, undefined, false, shortcutLabel("selectAll", mac))}
      {item("Find…", <Search size={12} />, props.onFind, "terminal.find")}
      {item("Copy Mode", <MousePointer2 size={12} />, props.onCopyMode, "terminal.copyMode")}
      {item("Clear", <Eraser size={12} />, props.onClear, "terminal.clear")}
    </div>,
    document.body,
  );
}
