import { useEffect, useCallback, useRef } from "react";

export interface KeyBinding {
  key: string;
  ctrl?: boolean;
  meta?: boolean;  // Cmd on Mac, Ctrl on Windows/Linux
  shift?: boolean;
  alt?: boolean;
  handler: () => void;
  description?: string;
}

/**
 * Register global keyboard shortcuts.
 * Bindings are matched against keydown events on the document.
 */
export function useKeyboard(bindings: KeyBinding[]) {
  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Don't intercept when typing in inputs (unless it's a meta/ctrl combo)
      const target = e.target as HTMLElement;
      const isInput = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;

      for (const binding of bindingsRef.current) {
        const metaMatch = binding.meta ? (e.metaKey || e.ctrlKey) : true;
        const ctrlMatch = binding.ctrl ? e.ctrlKey : true;
        const shiftMatch = binding.shift ? e.shiftKey : !e.shiftKey;
        const altMatch = binding.alt ? e.altKey : !e.altKey;
        const keyMatch = e.key.toLowerCase() === binding.key.toLowerCase();

        // For non-modifier combos in inputs, skip
        if (isInput && !binding.meta && !binding.ctrl) continue;

        if (keyMatch && metaMatch && ctrlMatch && shiftMatch && altMatch) {
          // Extra check: if binding requires meta, ensure meta is pressed
          if (binding.meta && !(e.metaKey || e.ctrlKey)) continue;
          if (binding.ctrl && !e.ctrlKey) continue;

          e.preventDefault();
          e.stopPropagation();
          binding.handler();
          return;
        }
      }
    }

    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, []);
}

/** Simple Escape key handler */
export function useEscape(handler: () => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        handlerRef.current();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);
}
