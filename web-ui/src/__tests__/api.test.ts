/**
 * Unit tests for web-ui/src/api.ts
 *
 * Tests token management, fetch helpers (apiFetch/apiPost/apiDelete/apiPatch),
 * auth functions, and data-parsing edge cases.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  getToken,
  setToken,
  clearToken,
  login,
  verifyToken,
  fetchAppState,
  fetchSessionMessages,
  fetchProviders,
  parseOpenCodeEvent,
  classifyFile,
  fetchGitDiff,
  fetchGitLog,
  searchMessages,
  rawFileUrl,
} from "../api";

// ── Mock sessionStorage ────────────────────────────────
const storage = new Map<string, string>();
const sessionStorageMock: Storage = {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => { storage.set(key, value); },
  removeItem: (key: string) => { storage.delete(key); },
  clear: () => storage.clear(),
  get length() { return storage.size; },
  key: (_i: number) => null,
};
Object.defineProperty(globalThis, "sessionStorage", { value: sessionStorageMock, writable: true });

// ── Mock window.location.reload ────────────────────────
const reloadMock = vi.fn();
Object.defineProperty(globalThis, "window", {
  value: { location: { reload: reloadMock } },
  writable: true,
});

// ── Helpers ─────────────────────────────────────────────
function mockFetch(response: Partial<Response> & { json?: () => Promise<unknown>; text?: () => Promise<string> }) {
  const defaults = {
    ok: true,
    status: 200,
    statusText: "OK",
    json: () => Promise.resolve({}),
    text: () => Promise.resolve(""),
  };
  globalThis.fetch = vi.fn().mockResolvedValue({ ...defaults, ...response });
}

beforeEach(() => {
  storage.clear();
  reloadMock.mockClear();
  vi.restoreAllMocks();
});

// ═══════════════════════════════════════════════════════
// Token management
// ═══════════════════════════════════════════════════════

describe("Token management", () => {
  it("getToken returns null when no token set", () => {
    expect(getToken()).toBeNull();
  });

  it("setToken / getToken round-trips", () => {
    setToken("abc123");
    expect(getToken()).toBe("abc123");
  });

  it("clearToken removes the token", () => {
    setToken("abc123");
    clearToken();
    expect(getToken()).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════
// Auth
// ═══════════════════════════════════════════════════════

describe("login", () => {
  it("returns token on success", async () => {
    mockFetch({
      ok: true,
      json: () => Promise.resolve({ token: "jwt.token.here" }),
    });
    const token = await login("user", "pass");
    expect(token).toBe("jwt.token.here");
    expect(fetch).toHaveBeenCalledWith("/api/auth/login", expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ username: "user", password: "pass" }),
    }));
  });

  it("throws on invalid credentials", async () => {
    mockFetch({ ok: false, status: 401 });
    await expect(login("user", "bad")).rejects.toThrow("Invalid credentials");
  });
});

describe("verifyToken", () => {
  it("returns false when no token stored", async () => {
    const result = await verifyToken();
    expect(result).toBe(false);
  });

  it("returns true when server verifies", async () => {
    setToken("valid_token");
    mockFetch({ ok: true });
    const result = await verifyToken();
    expect(result).toBe(true);
    expect(fetch).toHaveBeenCalledWith("/api/auth/verify", expect.objectContaining({
      headers: { Authorization: "Bearer valid_token" },
    }));
  });

  it("returns false when server rejects", async () => {
    setToken("expired_token");
    mockFetch({ ok: false, status: 401 });
    const result = await verifyToken();
    expect(result).toBe(false);
  });

  it("returns false on network error", async () => {
    setToken("some_token");
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("network"));
    const result = await verifyToken();
    expect(result).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════
// apiFetch (tested via fetchAppState)
// ═══════════════════════════════════════════════════════

describe("apiFetch (via fetchAppState)", () => {
  it("attaches auth header and returns parsed JSON", async () => {
    setToken("my_token");
    const mockState = { projects: [], active_project: 0, panels: {}, focused: "chat" };
    mockFetch({ ok: true, json: () => Promise.resolve(mockState) });

    const state = await fetchAppState();
    expect(state).toEqual(mockState);
    expect(fetch).toHaveBeenCalledWith("/api/state", expect.objectContaining({
      headers: expect.objectContaining({
        Authorization: "Bearer my_token",
        "Content-Type": "application/json",
      }),
    }));
  });

  it("clears token and throws on 401", async () => {
    setToken("old_token");
    mockFetch({ ok: false, status: 401 });
    await expect(fetchAppState()).rejects.toThrow("Unauthorized");
    expect(getToken()).toBeNull();
    expect(reloadMock).toHaveBeenCalled();
  });

  it("throws on non-401 errors", async () => {
    mockFetch({ ok: false, status: 500, statusText: "Internal Server Error" });
    await expect(fetchAppState()).rejects.toThrow("API error: 500 Internal Server Error");
  });
});

// ═══════════════════════════════════════════════════════
// fetchSessionMessages (data parsing edge cases)
// ═══════════════════════════════════════════════════════

describe("fetchSessionMessages", () => {
  it("extracts from { messages: [...] } format", async () => {
    const msgs = [{ info: { role: "user" }, parts: [] }];
    mockFetch({ ok: true, json: () => Promise.resolve({ messages: msgs }) });
    const result = await fetchSessionMessages("sid1");
    expect(result).toEqual(msgs);
  });

  it("handles legacy object-keyed format", async () => {
    const msg1 = { info: { role: "user" }, parts: [] };
    mockFetch({ ok: true, json: () => Promise.resolve({ msg1 }) });
    const result = await fetchSessionMessages("sid1");
    expect(result).toEqual([msg1]);
  });

  it("handles plain array format", async () => {
    const msgs = [{ info: { role: "user" }, parts: [] }];
    mockFetch({ ok: true, json: () => Promise.resolve(msgs) });
    const result = await fetchSessionMessages("sid1");
    expect(result).toEqual(msgs);
  });

  it("returns empty array for unexpected format", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve(42) });
    const result = await fetchSessionMessages("sid1");
    expect(result).toEqual([]);
  });
});

// ═══════════════════════════════════════════════════════
// fetchProviders (response normalization)
// ═══════════════════════════════════════════════════════

describe("fetchProviders", () => {
  it("normalizes the { all, connected, default } response", async () => {
    const resp = {
      all: [{ id: "openai", name: "OpenAI", models: [] }],
      connected: ["openai"],
      default: { chat: "gpt-4" },
    };
    mockFetch({ ok: true, json: () => Promise.resolve(resp) });
    const result = await fetchProviders();
    expect(result.all).toHaveLength(1);
    expect(result.connected).toEqual(["openai"]);
    expect(result.default).toEqual({ chat: "gpt-4" });
  });

  it("handles flat array fallback", async () => {
    const providers = [{ id: "anthropic" }];
    mockFetch({ ok: true, json: () => Promise.resolve(providers) });
    const result = await fetchProviders();
    expect(result.all).toEqual(providers);
    expect(result.connected).toEqual([]);
  });

  it("handles empty/unexpected response", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve(null) });
    const result = await fetchProviders();
    expect(result.all).toEqual([]);
    expect(result.connected).toEqual([]);
    expect(result.default).toEqual({});
  });
});

// ═══════════════════════════════════════════════════════
// parseOpenCodeEvent
// ═══════════════════════════════════════════════════════

describe("parseOpenCodeEvent", () => {
  it("parses envelope format { payload: { type, properties } }", () => {
    const data = JSON.stringify({
      directory: "/tmp",
      payload: { type: "message.updated", properties: { info: {} } },
    });
    const event = parseOpenCodeEvent(data);
    expect(event).toEqual({ type: "message.updated", properties: { info: {} } });
  });

  it("parses top-level format { type, properties }", () => {
    const data = JSON.stringify({ type: "session.created", properties: {} });
    const event = parseOpenCodeEvent(data);
    expect(event?.type).toBe("session.created");
  });

  it("returns null for invalid JSON", () => {
    expect(parseOpenCodeEvent("not json")).toBeNull();
  });

  it("returns null for object without type", () => {
    expect(parseOpenCodeEvent(JSON.stringify({ foo: "bar" }))).toBeNull();
  });
});

// ═══════════════════════════════════════════════════════
// classifyFile
// ═══════════════════════════════════════════════════════

describe("classifyFile", () => {
  it.each([
    ["photo.png", "image"],
    ["photo.jpg", "image"],
    ["photo.webp", "image"],
    ["audio.mp3", "audio"],
    ["audio.wav", "audio"],
    ["movie.mp4", "video"],
    ["movie.webm", "video"],
    ["doc.pdf", "pdf"],
    ["data.csv", "csv"],
    ["readme.md", "markdown"],
    ["readme.mdx", "markdown"],
    ["page.html", "html"],
    ["diagram.mermaid", "mermaid"],
    ["archive.zip", "binary"],
    ["lib.wasm", "binary"],
    ["main.ts", "code"],
    ["app.py", "code"],
    ["style.css", "code"],
    ["noext", "code"],
  ])("classifies %s as %s", (path, expected) => {
    expect(classifyFile(path)).toBe(expected);
  });
});

// ═══════════════════════════════════════════════════════
// URL building (fetchGitDiff, fetchGitLog, rawFileUrl, searchMessages)
// ═══════════════════════════════════════════════════════

describe("URL building", () => {
  it("fetchGitDiff builds correct query string", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve({ diff: "..." }) });
    await fetchGitDiff("src/main.rs", true);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/git/diff?file=src%2Fmain.rs&staged=true"),
      expect.anything(),
    );
  });

  it("fetchGitDiff with no args has no query string", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve({ diff: "" }) });
    await fetchGitDiff();
    expect(fetch).toHaveBeenCalledWith("/api/git/diff", expect.anything());
  });

  it("fetchGitLog includes limit", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve({ commits: [] }) });
    await fetchGitLog(50);
    expect(fetch).toHaveBeenCalledWith("/api/git/log?limit=50", expect.anything());
  });

  it("rawFileUrl includes path and token", () => {
    setToken("tok");
    const url = rawFileUrl("src/foo.rs");
    expect(url).toContain("path=src%2Ffoo.rs");
    expect(url).toContain("token=tok");
  });

  it("searchMessages builds correct query", async () => {
    mockFetch({ ok: true, json: () => Promise.resolve({ query: "test", results: [], total: 0 }) });
    await searchMessages(0, "hello world", 10);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/project/0/search?q=hello+world&limit=10"),
      expect.anything(),
    );
  });
});
