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

/** Typed GET fetch helper */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status} ${res.statusText}`);
  return res.json();
}

/** POST helper */
export async function apiPost<T = void>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  const text = await res.text();
  if (text) return JSON.parse(text) as T;
  return undefined as unknown as T;
}

/** DELETE helper */
export async function apiDelete(path: string): Promise<void> {
  const res = await fetch(`/api${path}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
}

/** PATCH helper */
export async function apiPatch<T = void>(
  path: string,
  body?: unknown
): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "PATCH",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  const text = await res.text();
  if (text) return JSON.parse(text) as T;
  return undefined as unknown as T;
}

/** PUT helper */
export async function apiPut<T = void>(
  path: string,
  body?: unknown
): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "PUT",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
    throw new Error("Unauthorized");
  }
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  const text = await res.text();
  if (text) return JSON.parse(text) as T;
  return undefined as unknown as T;
}
