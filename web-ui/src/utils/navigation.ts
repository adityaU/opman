/** Single entry point for app-level history navigation.
 *
 *  `history.pushState`/`replaceState` do NOT emit any event, so independent
 *  hooks that each derive view state from `window.location` can fall out of
 *  sync when another hook navigates (e.g. selecting a session must also take
 *  the Kanban view-state out of the board). Routing every navigation through
 *  `appNavigate` and emitting a `locationchange` event lets all location-derived
 *  hooks recompute from one source of truth. */
export const LOCATION_CHANGE_EVENT = "opman:locationchange";

/** Push (or replace) a URL and notify location-derived hooks. */
export function appNavigate(url: string, opts?: { replace?: boolean }): void {
  if (opts?.replace) {
    window.history.replaceState(null, "", url);
  } else {
    window.history.pushState(null, "", url);
  }
  window.dispatchEvent(new Event(LOCATION_CHANGE_EVENT));
}

/** Subscribe to both back/forward (`popstate`) and programmatic
 *  (`appNavigate`) location changes. Returns an unsubscribe fn. */
export function onLocationChange(handler: () => void): () => void {
  window.addEventListener("popstate", handler);
  window.addEventListener(LOCATION_CHANGE_EVENT, handler);
  return () => {
    window.removeEventListener("popstate", handler);
    window.removeEventListener(LOCATION_CHANGE_EVENT, handler);
  };
}
