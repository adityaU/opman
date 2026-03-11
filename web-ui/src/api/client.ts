import type {
  Message,
  Provider,
  SlashCommand,
  TodoItem,
  OpenCodeEvent,
} from "../types";

// ── Token management ──────────────────────────────────

/** Get the stored auth token */
export function getToken(): string | null {
  return sessionStorage.getItem("opman_token");
}

/** Store auth token */
export function setToken(token: string) {
  sessionStorage.setItem("opman_token", token);
}

/** Clear auth token */
export function clearToken() {
  sessionStorage.removeItem("opman_token");
}

/** Build auth headers */
export function authHeaders(): Record<string, string> {
  const token = getToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

/** Typed GET fetch helper */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
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
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
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
    headers: { ...authHeaders() },
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
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
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
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
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
