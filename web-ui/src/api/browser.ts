import { apiFetch, apiPost } from "./client";

/**
 * Browser panes.
 *
 * A pane is a real headless Chromium tab living in the opman process. The same
 * tab is what the `browser` MCP tools drive, so an agent working the page and a
 * person watching it are looking at one thing rather than two that have to be
 * kept in step.
 */

/**
 * How the pane draws the page. Decided by the server from the site's framing
 * headers, because a page inside an iframe cannot read back why it was refused.
 */
export type BrowserMode = "iframe" | "screencast";

export interface BrowserPage {
  readonly paneId: string;
  readonly project?: string;
  readonly mode?: BrowserMode;
  readonly url: string;
  readonly title: string;
  /**
   * True when the pane connected to a tab that was already running — so the URL
   * is where the browser actually is, which may be somewhere an agent drove it
   * rather than where this widget was last saved.
   */
  readonly adopted?: boolean;
}

/** The compact `[ref=eN]` outline — the same text the LLM reads. */
export interface BrowserSnapshot {
  readonly url: string;
  readonly title: string;
  readonly scroll_y: number;
  readonly scroll_height: number;
  readonly viewport_height: number;
  readonly ref_count: number;
  readonly truncated: boolean;
  readonly outline: string;
}

export interface BrowserPaneInfo {
  readonly pane_id: string;
  readonly project: string;
  readonly url: string;
  readonly title: string;
  readonly mode: BrowserMode;
}

/**
 * Connect the pane to its browser, sending it to `url` only if the tab is new.
 *
 * The tab outlives the widget, so reopening a browser pane must adopt whatever
 * page is live rather than steering it back to a saved one.
 */
export function browserOpen(
  paneId: string,
  project: string,
  url?: string,
): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/open", { pane_id: paneId, project, url });
}

export function browserNavigate(
  paneId: string,
  project: string,
  url: string,
): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/navigate", { pane_id: paneId, project, url });
}

/**
 * The browser id for a project. Derived, and derived identically in the Rust
 * backend, so a browser an agent opens for a repo is the browser this pane
 * connects to — neither side has to be told the other's id.
 */
export function browserIdForProject(projectPath: string): string {
  return `proj:${projectPath}`;
}

export function browserBack(paneId: string): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/back", { pane_id: paneId });
}

export function browserForward(paneId: string): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/forward", { pane_id: paneId });
}

export function browserReload(paneId: string): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/reload", { pane_id: paneId });
}

export function browserSnapshot(paneId: string): Promise<BrowserSnapshot> {
  return apiFetch<BrowserSnapshot>(`/browser/snapshot?pane_id=${encodeURIComponent(paneId)}`);
}

export function browserSetMode(paneId: string, mode: BrowserMode): Promise<BrowserPage> {
  return apiPost<BrowserPage>("/browser/mode", { pane_id: paneId, mode });
}

export function browserResize(paneId: string, width: number, height: number): Promise<void> {
  return apiPost("/browser/resize", { pane_id: paneId, width, height });
}

export function browserClose(paneId: string): Promise<void> {
  return apiPost("/browser/close", { pane_id: paneId });
}

export function browserList(): Promise<{ panes: readonly BrowserPaneInfo[] }> {
  return apiFetch<{ panes: readonly BrowserPaneInfo[] }>("/browser/list");
}

// ── Screencast input ──────────────────────────────────
//
// Only used in screencast mode; an iframe delivers its own events natively.

export type BrowserMouseKind = "move" | "down" | "up";

export function browserMouse(
  paneId: string,
  kind: BrowserMouseKind,
  x: number,
  y: number,
): Promise<void> {
  return apiPost("/browser/mouse", { pane_id: paneId, kind, x, y });
}

export function browserKey(paneId: string, key: string): Promise<void> {
  return apiPost("/browser/key", { pane_id: paneId, key });
}

export function browserTextInput(paneId: string, text: string): Promise<void> {
  return apiPost("/browser/text-input", { pane_id: paneId, text });
}

export function browserScroll(paneId: string, deltaY: number, x = 0, y = 0): Promise<void> {
  return apiPost("/browser/scroll", { pane_id: paneId, x, y, delta_y: deltaY });
}

/**
 * Live frames. Cookie auth means no token on the query string, matching every
 * other EventSource in the app.
 */
export function createBrowserFrameSSE(paneId: string): EventSource {
  return new EventSource(`/api/browser/stream?pane_id=${encodeURIComponent(paneId)}`, {
    withCredentials: true,
  });
}
