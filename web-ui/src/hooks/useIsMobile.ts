/**
 * useIsMobile — one answer to "is this a phone-sized viewport", shared by
 * every component that needs it.
 *
 * There used to be four copies of this check with three different comparators
 * (`>= 768`, `< 768`, `<= 768`) against the same number, and none of them
 * matched the stylesheets, which use `max-width: 768px` — 768 inclusive. At
 * exactly 768px wide, the common iPad portrait width, CSS showed the mobile
 * dock while JavaScript believed it was on desktop. The editor mounted inside
 * the mobile sheet then rendered its desktop layout.
 *
 * So the predicate lives here, phrased exactly as the CSS phrases it, and the
 * media query itself is the source of truth rather than a number compared by
 * hand.
 */
import { useEffect, useState } from "react";

/** Must stay in step with the `max-width` in responsive-*.css. */
export const MOBILE_QUERY = "(max-width: 768px)";

function matches(): boolean {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia(MOBILE_QUERY).matches;
}

export function useIsMobile(): boolean {
  const [isMobile, setIsMobile] = useState(matches);

  useEffect(() => {
    const query = window.matchMedia(MOBILE_QUERY);
    const onChange = (event: MediaQueryListEvent) => setIsMobile(event.matches);
    // Re-read on mount: the viewport can change between first render and here.
    setIsMobile(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  return isMobile;
}

/**
 * One-shot check for code outside React — event handlers and callbacks that
 * only need the answer at the moment they run.
 */
export function isMobileViewport(): boolean {
  return matches();
}
