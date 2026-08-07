import { useEffect } from "react";
import { useKeymapContext } from "./KeymapContext";

/**
 * Publishes which surface has focus, for `focus==` clauses.
 *
 * Derived from the DOM rather than from React state: focus is already a DOM
 * concept, and a panel that marks itself with `data-surface` gets its bare keys
 * without threading a callback through every component between it and the
 * layout. One listener answers for every surface, present and future.
 */

export const SURFACE_ATTRIBUTE = "data-surface";

function surfaceOf(node: EventTarget | null): string | undefined {
  if (!(node instanceof Element)) return undefined;
  const owner = node.closest(`[${SURFACE_ATTRIBUTE}]`);
  return owner?.getAttribute(SURFACE_ATTRIBUTE) ?? undefined;
}

export function useSurfaceFocus(): void {
  const { setContext } = useKeymapContext();

  useEffect(() => {
    const publish = (target: EventTarget | null) =>
      setContext({ focus: surfaceOf(target) ?? "chat" });

    // `focusin` bubbles where `focus` does not, so one document listener covers
    // every panel. Pointer-down is tracked too: clicking a panel's background
    // moves the user's attention even when nothing focusable receives it.
    const onFocusIn = (event: FocusEvent) => publish(event.target);
    const onPointerDown = (event: PointerEvent) => publish(event.target);

    document.addEventListener("focusin", onFocusIn, true);
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      document.removeEventListener("focusin", onFocusIn, true);
      document.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [setContext]);
}
