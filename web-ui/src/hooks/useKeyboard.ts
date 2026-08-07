import { useEffect, useRef } from "react";

/**
 * Simple Escape key handler.
 *
 * Registered in the capture phase, because the keymap listener is also a
 * capture-phase listener on `document` and calls `stopPropagation()` when it
 * consumes a key — which cancels the bubble phase entirely. A listener on the
 * same node still runs, so capture is what makes Escape reach surfaces that own
 * their open state locally rather than through the modal registry.
 */
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
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, []);
}
