/** Single entry point for app-level history navigation.
 *
 *  `history.pushState`/`replaceState` do NOT emit any event, so independent
 *  hooks that each derive view state from `window.location` can fall out of
 *  sync when another hook navigates (e.g. selecting a session must also take
 *  the Kanban view-state out of the board). Routing every navigation through
 *  `appNavigate` and emitting a `locationchange` event lets all location-derived
 *  hooks recompute from one source of truth. */
export const LOCATION_CHANGE_EVENT = "opman:locationchange";

/**
 * The app's destinations other than chat.
 *
 * Owned here rather than by each route's own hook because the session-restoring hooks
 * have to know "am I on a page that is not the chat" *before* any of those hooks run —
 * without it they write `/?session=…` over whatever path the user actually opened, and
 * the page bounces to a conversation. That was a one-off special case for the board; a
 * table is what stops the next route rediscovering it.
 */
export const KANBAN_PATH = "/kanban";
export const SETTINGS_PATH = "/settings";

const PAGE_PATHS: readonly string[] = [KANBAN_PATH, SETTINGS_PATH];

/** Whether the URL names a page of its own rather than the chat view. */
export function onPageRoute(): boolean {
  return PAGE_PATHS.some((path) => window.location.pathname.startsWith(path));
}

/**
 * Sentinel `useModalState` puts on the history entries it pushes.
 *
 * Shared here because a navigation has to recognise one: a modal's entry is a throwaway
 * that exists only so the back gesture can dismiss it, and stacking a page on top of it
 * would cost the user two back presses to return to where they started.
 */
export const MODAL_HISTORY_KEY = "_modalLayer";

function onModalHistoryEntry(): boolean {
  const state = window.history.state as Record<string, unknown> | null;
  return Boolean(state && MODAL_HISTORY_KEY in state);
}

/** Push (or replace) a URL and notify location-derived hooks. */
export function appNavigate(url: string, opts?: { replace?: boolean }): void {
  // A modal's entry is replaced rather than built on: the surface that pushed it is being
  // left behind, so it should not become a stop on the way back.
  if (opts?.replace || onModalHistoryEntry()) {
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
