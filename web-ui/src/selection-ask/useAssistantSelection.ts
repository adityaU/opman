import { useCallback, useEffect, useState, type RefObject } from "react";

/** Text the user selected inside one assistant reply, and where it sits on screen. */
export interface AssistantSelection {
  readonly text: string;
  readonly rect: DOMRect;
}

const ASSISTANT_TURN = ".message-turn-assistant";

/** The assistant turn an element sits in, if any, within `root`. */
function turnOf(node: Node | null, root: HTMLElement): Element | null {
  const element = node instanceof Element ? node : node?.parentElement ?? null;
  const turn = element?.closest(ASSISTANT_TURN) ?? null;
  return turn && root.contains(turn) ? turn : null;
}

/** The selection, when it lies wholly inside one assistant reply of `root`. */
export function readAssistantSelection(root: HTMLElement): AssistantSelection | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const text = selection.toString();
  if (!text.trim()) return null;
  const turn = turnOf(selection.anchorNode, root);
  if (!turn || turn !== turnOf(selection.focusNode, root)) return null;
  const rect = selection.getRangeAt(0).getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return null;
  return { text, rect };
}

/**
 * Watch for a finished selection inside an assistant reply of the timeline.
 *
 * Read when the pointer or a key is released rather than on every `selectionchange`,
 * so the bar appears once the user has finished dragging, not under their cursor while
 * they still are. Scrolling hides it: its position would no longer match the text.
 * `frozen` keeps the current one while the user types a question about it — focusing
 * that field clears the page selection, which must not take the bar away.
 */
export function useAssistantSelection(
  root: RefObject<HTMLElement | null>,
  frozen: boolean,
): [AssistantSelection | null, () => void] {
  const [current, setCurrent] = useState<AssistantSelection | null>(null);
  const clear = useCallback(() => setCurrent(null), []);

  useEffect(() => {
    if (frozen) return;
    const container = root.current;
    if (!container) return;
    // After the browser has settled the selection for this gesture.
    const read = () => window.setTimeout(() => setCurrent(readAssistantSelection(container)), 0);
    const hide = () => setCurrent(null);
    container.addEventListener("mouseup", read);
    container.addEventListener("keyup", read);
    container.addEventListener("touchend", read);
    container.addEventListener("scroll", hide, { passive: true });
    return () => {
      container.removeEventListener("mouseup", read);
      container.removeEventListener("keyup", read);
      container.removeEventListener("touchend", read);
      container.removeEventListener("scroll", hide);
    };
  }, [root, frozen]);

  return [current, clear];
}
