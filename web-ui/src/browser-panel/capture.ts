/**
 * Who owns the keyboard while a browser pane has focus.
 *
 * The app keymap listens in the capture phase, so without this it sees every
 * keystroke first and a page in a pane cannot be typed into: `/` opens the
 * palette instead of a site's search box, `g` runs a chord, Cmd+F finds in the
 * app rather than in the page. A pane showing a real browser has to behave like
 * a browser, so while its surface has focus the keymap stands down completely
 * and every key is forwarded to the page.
 *
 * The way back out is a double Escape, handled by the surface itself: the first
 * one goes to the page (sites use it to close their own dialogs), and a second
 * one in quick succession releases focus. That is the only key the app keeps,
 * and it is only kept on the second press.
 */

export const BROWSER_CAPTURE_ATTRIBUTE = "data-browser-capture";

/** How close two Escapes must be to mean "give me the app back". */
export const RELEASE_WINDOW_MS = 500;

/**
 * True while the event is headed for a browser surface.
 *
 * No focus check is needed and none is done: a keydown is dispatched at the
 * focused element, so an event whose target sits inside a surface *is* an event
 * that surface has focus for. A pane in the background never matches, and the
 * mark can therefore stay on the element permanently instead of being toggled on
 * focus — one less piece of state to be a frame out of date.
 */
export function browserOwnsKey(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest(`[${BROWSER_CAPTURE_ATTRIBUTE}]`) !== null;
}

/**
 * Whether this Escape is the second of a pair. Kept as a pure function over the
 * previous timestamp so the surface holds the state and this stays testable.
 */
export function releasesKeyboard(
  key: string,
  previousEscapeAt: number | undefined,
  now: number,
): boolean {
  if (key !== "Escape") return false;
  if (previousEscapeAt === undefined) return false;
  return now - previousEscapeAt <= RELEASE_WINDOW_MS;
}
