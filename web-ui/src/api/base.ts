/**
 * Which opman server this frontend instance talks to.
 *
 * One page load is one server. The page that home serves at `/` talks to home; the same
 * SPA served at `/remote/<id>/…` talks to remote `<id>` through home's proxy at
 * `/remote/<id>/api/…`. Everything here is derived once from the URL the page was opened
 * at — switching server is a navigation, never a runtime toggle, so no caller has to cope
 * with the base changing under it.
 *
 * Contract: docs/multi-server-device-browser.md ("Frontend contract").
 */

/** The id home goes by in `/api/servers` and in widget `server` fields. */
export const HOME_SERVER_ID = "home";

const REMOTE_PREFIX = "/remote/";

/** The `/remote/<id>` prefix a pathname starts with, or `""` for home. */
export function detectBase(pathname: string): string {
  if (!pathname.startsWith(REMOTE_PREFIX)) return "";
  const id = pathname.slice(REMOTE_PREFIX.length).split("/")[0];
  if (!id) return "";
  return `${REMOTE_PREFIX}${id}`;
}

/** The server id a base names: `"home"` for `""`, else the `<id>` segment. */
export function serverIdOf(base: string): string {
  if (!base) return HOME_SERVER_ID;
  return decodeURIComponent(base.slice(REMOTE_PREFIX.length));
}

function currentPathname(): string {
  return typeof window === "undefined" ? "/" : window.location.pathname;
}

/** `""` on home, `"/remote/<id>"` on a remote instance. */
export const BASE: string = detectBase(currentPathname());

/** `"home"` or the remote's id. */
export const SERVER_ID: string = serverIdOf(BASE);

/** True when this instance is home rather than a proxied remote. */
export const IS_HOME = SERVER_ID === HOME_SERVER_ID;

/** Every backend URL this instance builds: `${BASE}/api${path}`. */
export function apiUrl(path: string): string {
  return `${BASE}/api${path}`;
}

/** The path prefix a server's instance lives under: `""` for home. */
export function serverBase(id: string): string {
  if (!id || id === HOME_SERVER_ID) return "";
  return `${REMOTE_PREFIX}${encodeURIComponent(id)}`;
}

/**
 * Namespace a localStorage/sessionStorage key by server.
 *
 * Home keeps the bare key so existing users keep their state; a remote's state lives
 * under `key@<id>` so two instances in one browser never read each other's sessions.
 */
export function storageKey(key: string): string {
  return IS_HOME ? key : `${key}@${SERVER_ID}`;
}

/**
 * A same-origin URL the backend handed back (`/api/...`) made reachable from this
 * instance. Absolute URLs and ones already carrying the base pass through.
 */
export function resolveBackendUrl(url: string): string {
  if (!url.startsWith("/api/")) return url;
  return `${BASE}${url}`;
}

/** A path inside this instance (`/settings`) made into a real location path. */
export function appPath(path: string): string {
  if (!BASE || !path.startsWith("/") || path.startsWith(`${BASE}/`)) return path;
  return `${BASE}${path}`;
}

/** The current location's pathname with this instance's base removed. */
export function instancePathname(pathname: string = currentPathname()): string {
  if (!BASE || !pathname.startsWith(BASE)) return pathname;
  return pathname.slice(BASE.length) || "/";
}
