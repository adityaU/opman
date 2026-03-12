import { useEffect, useRef } from "react";

/**
 * Detects whether a virtual (software) keyboard is open on mobile devices
 * using the Visual Viewport API.
 *
 * When the keyboard is open:
 * - Sets `data-vkb-open` on `<html>` so CSS can reposition bottom-sheet
 *   modals to start from the top instead.
 * - Sets `--vkb-height` CSS custom property on `<html>` to the actual
 *   visible viewport height (px) so modals can cap their max-height to
 *   the space above the keyboard.
 *
 * Heuristic: the keyboard is considered open when `visualViewport.height`
 * is significantly smaller than `window.innerHeight` (threshold: 150 px).
 * This avoids false positives from address-bar collapse/expand.
 */
export function useVirtualKeyboard(): void {
  const openRef = useRef(false);

  useEffect(() => {
    const vv = window.visualViewport;
    if (!vv) return;

    const THRESHOLD = 150; // px difference to consider keyboard "open"

    function update() {
      if (!vv) return;
      // On mobile, window.innerHeight stays the same (full layout height),
      // while visualViewport.height shrinks when the keyboard is shown.
      const diff = window.innerHeight - vv.height;
      const isOpen = diff > THRESHOLD;

      if (isOpen !== openRef.current) {
        openRef.current = isOpen;
        if (isOpen) {
          document.documentElement.setAttribute("data-vkb-open", "");
        } else {
          document.documentElement.removeAttribute("data-vkb-open");
          document.documentElement.style.removeProperty("--vkb-height");
        }
      }

      // Continuously update the available height while keyboard is open
      // so modals track the actual visible area.
      if (isOpen) {
        document.documentElement.style.setProperty(
          "--vkb-height",
          `${Math.round(vv.height)}px`,
        );
      }
    }

    vv.addEventListener("resize", update);
    // Also listen to scroll events on visualViewport (iOS fires this
    // when the viewport is panned by the keyboard).
    vv.addEventListener("scroll", update);

    // Initial check
    update();

    return () => {
      vv.removeEventListener("resize", update);
      vv.removeEventListener("scroll", update);
      document.documentElement.removeAttribute("data-vkb-open");
      document.documentElement.style.removeProperty("--vkb-height");
      openRef.current = false;
    };
  }, []);
}
