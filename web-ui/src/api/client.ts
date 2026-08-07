import type {
  Message,
  Provider,
  SlashCommand,
  TodoItem,
  OpenCodeEvent,
} from "../types";

// ── Token management ──────────────────────────────────
//
// Auth is now cookie-based: the backend sets an HttpOnly `opman_token`
// cookie on login, and the browser sends it automatically with every
// same-origin request (fetch, EventSource, WebSocket upgrade, etc.).
//
// These helpers are retained for backward-compat / edge-cases but
// sessionStorage is no longer the source of truth.

/** @deprecated Cookie-based auth means the browser handles token storage. */
export function getToken(): string | null {
  // Still check sessionStorage for the transition period — if any old
  // code stored a token there it will be picked up, but new logins
  // rely solely on the cookie set by the server.
  return sessionStorage.getItem("opman_token");
}

/** @deprecated Token is now set as HttpOnly cookie by the backend. */
export function setToken(_token: string) {
  // No-op: the server sets an HttpOnly cookie on login.
  // Clean up any legacy sessionStorage entry.
  sessionStorage.removeItem("opman_token");
}

/** Clear any leftover sessionStorage token. */
export function clearToken() {
  sessionStorage.removeItem("opman_token");
}

/** Build auth headers — empty object when using cookie auth. */
export function authHeaders(): Record<string, string> {
  // The cookie is sent automatically by the browser. No need for
  // explicit Authorization headers on same-origin requests.
  return {};
}

/**
 * One place that turns a response into a value or an error.
 *
 * Every helper below used to repeat the 401-reload and the ok-check, and they had drifted:
 * only `apiPost` read the `{ "error": … }` body the backend always sends, so a failed PUT
 * surfaced as a bare status code and the reason never reached the user.
 *
 * A response with no body is a success for a write and a failure for a read, so decoding
 * stays split between [`readJson`] and [`maybeJson`] rather than being unified into a
 * lenient middle that would turn an empty read into `undefined`.
 */
async function guard(res: Response): Promise<Response> {
  if (res.status === 401) {
    // The cookie is gone or expired. Reloading lands on the login page.
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(await detail(res));
  return res;
}

/**
 * The server's own message for a failure, falling back to the status.
 *
 * A non-JSON body is only used when it reads like a message. A route the backend does not
 * have answers with the SPA's `index.html`, and showing a user a page of markup instead of
 * "404 Not Found" is worse than showing them nothing.
 */
async function detail(res: Response): Promise<string> {
  const raw = (await res.text().catch(() => "")).trim();
  try {
    const body = JSON.parse(raw);
    if (body?.error || body?.message) return body.error || body.message;
  } catch {
    const prose = raw.length <= 200 && !raw.startsWith("<");
    if (prose && raw) return raw;
  }
  return `API error: ${res.status} ${res.statusText}`.trimEnd();
}

/**
 * A body that must be there.
 *
 * A 200 that is not JSON means the request never reached the handler — the SPA fallback,
 * a proxy, a captive portal. Saying so beats surfacing the parser's "Unexpected token '<'".
 */
async function readJson<T>(res: Response): Promise<T> {
  try {
    return (await res.json()) as T;
  } catch {
    throw new Error("API error: the server did not return JSON");
  }
}

/** A body that may legitimately be empty, as a write's often is. */
async function maybeJson<T>(res: Response): Promise<T> {
  const text = await res.text();
  if (text) return JSON.parse(text) as T;
  return undefined as unknown as T;
}

function json(method: string, body?: unknown): RequestInit {
  return {
    method,
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  };
}

/** Typed GET fetch helper. `init` may override the method for a custom request. */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", ...init?.headers },
  });
  return readJson<T>(await guard(res));
}

/** POST helper */
export async function apiPost<T = void>(path: string, body?: unknown): Promise<T> {
  return maybeJson<T>(await guard(await fetch(`/api${path}`, json("POST", body))));
}

/** DELETE helper */
export async function apiDelete(path: string): Promise<void> {
  await guard(await fetch(`/api${path}`, { method: "DELETE", credentials: "same-origin" }));
}

/** PATCH helper */
export async function apiPatch<T = void>(path: string, body?: unknown): Promise<T> {
  return maybeJson<T>(await guard(await fetch(`/api${path}`, json("PATCH", body))));
}

/** PUT helper */
export async function apiPut<T = void>(path: string, body?: unknown): Promise<T> {
  return maybeJson<T>(await guard(await fetch(`/api${path}`, json("PUT", body))));
}

/**
 * POST a multipart body.
 *
 * No `Content-Type` header: the browser must set it itself so the multipart boundary
 * matches the body it generated.
 */
export async function apiUpload<T = void>(path: string, body: FormData): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "POST",
    credentials: "same-origin",
    body,
  });
  return maybeJson<T>(await guard(res));
}
