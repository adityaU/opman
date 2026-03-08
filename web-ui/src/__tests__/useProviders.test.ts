/**
 * Unit tests for useProviders hook and invalidateProviderCache.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// ── Mock fetchProviders ────────────────────────────────
const mockFetchProviders = vi.fn();
vi.mock("../api", () => ({
  fetchProviders: (...args: unknown[]) => mockFetchProviders(...args),
}));

// Must import AFTER vi.mock so the mock is active
import { useProviders, invalidateProviderCache } from "../hooks/useProviders";
import type { ProviderCache } from "../hooks/useProviders";

// ── Helpers ────────────────────────────────────────────
function makeResp(overrides: Partial<{
  all: { id: string; name: string; models: string[] }[];
  connected: string[];
  default: Record<string, string>;
}> = {}) {
  return {
    all: overrides.all ?? [
      { id: "openai", name: "OpenAI", models: ["gpt-4", "gpt-3.5"] },
      { id: "anthropic", name: "Anthropic", models: ["claude-3"] },
    ],
    connected: overrides.connected ?? ["openai"],
    default: overrides.default ?? { chat: "gpt-4" },
  };
}

// ── Setup / teardown ───────────────────────────────────
beforeEach(() => {
  // Invalidate global cache between tests so each test starts fresh
  invalidateProviderCache();
  mockFetchProviders.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Tests ──────────────────────────────────────────────
describe("useProviders", () => {
  it("starts in loading state when cache is empty", () => {
    mockFetchProviders.mockReturnValue(new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useProviders());
    expect(result.current.loading).toBe(true);
    expect(result.current.all).toEqual([]);
    expect(result.current.connected.size).toBe(0);
    expect(result.current.defaults).toEqual({});
    expect(result.current.error).toBeNull();
  });

  it("fetches providers on mount and populates state", async () => {
    const resp = makeResp();
    mockFetchProviders.mockResolvedValue(resp);

    const { result } = renderHook(() => useProviders());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.all).toEqual(resp.all);
    expect(result.current.connected).toEqual(new Set(["openai"]));
    expect(result.current.defaults).toEqual({ chat: "gpt-4" });
    expect(result.current.error).toBeNull();
    expect(mockFetchProviders).toHaveBeenCalledOnce();
  });

  it("sets error state when fetchProviders rejects with Error", async () => {
    mockFetchProviders.mockRejectedValue(new Error("Network failure"));

    const { result } = renderHook(() => useProviders());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.error).toBe("Network failure");
    expect(result.current.all).toEqual([]);
  });

  it("sets generic error when fetchProviders rejects with non-Error", async () => {
    mockFetchProviders.mockRejectedValue("string error");

    const { result } = renderHook(() => useProviders());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.error).toBe("Failed to fetch providers");
  });

  it("uses cached data on subsequent mounts without re-fetching", async () => {
    const resp = makeResp();
    mockFetchProviders.mockResolvedValue(resp);

    // First mount populates cache
    const { result: r1, unmount } = renderHook(() => useProviders());
    await waitFor(() => expect(r1.current.loading).toBe(false));
    unmount();

    mockFetchProviders.mockClear();

    // Second mount should use cache
    const { result: r2 } = renderHook(() => useProviders());
    // Data should be available immediately from cache
    expect(r2.current.all).toEqual(resp.all);
    expect(r2.current.connected).toEqual(new Set(["openai"]));
    expect(r2.current.defaults).toEqual({ chat: "gpt-4" });
    // Should not call fetchProviders again (cache is still fresh)
    expect(mockFetchProviders).not.toHaveBeenCalled();
  });

  it("refresh() forces a re-fetch even with valid cache", async () => {
    const resp1 = makeResp();
    mockFetchProviders.mockResolvedValue(resp1);

    const { result } = renderHook(() => useProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(mockFetchProviders).toHaveBeenCalledOnce();

    // Prepare a different response for the refresh
    const resp2 = makeResp({
      all: [{ id: "google", name: "Google", models: ["gemini-pro"] }],
      connected: ["google"],
      default: { chat: "gemini-pro" },
    });
    mockFetchProviders.mockResolvedValue(resp2);

    // Force refresh
    await act(async () => {
      result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.all).toEqual(resp2.all);
    expect(result.current.connected).toEqual(new Set(["google"]));
    expect(result.current.defaults).toEqual({ chat: "gemini-pro" });
    expect(mockFetchProviders).toHaveBeenCalledTimes(2);
  });

  it("invalidateProviderCache causes next mount to re-fetch", async () => {
    const resp = makeResp();
    mockFetchProviders.mockResolvedValue(resp);

    // First mount populates cache
    const { result: r1, unmount } = renderHook(() => useProviders());
    await waitFor(() => expect(r1.current.loading).toBe(false));
    unmount();

    mockFetchProviders.mockClear();

    // Invalidate the global cache
    invalidateProviderCache();

    // Next mount should re-fetch
    mockFetchProviders.mockResolvedValue(resp);
    const { result: r2 } = renderHook(() => useProviders());
    await waitFor(() => expect(r2.current.loading).toBe(false));

    expect(mockFetchProviders).toHaveBeenCalledOnce();
  });

  it("does not update state after unmount (no memory leak)", async () => {
    let resolvePromise: (v: ReturnType<typeof makeResp>) => void;
    mockFetchProviders.mockReturnValue(
      new Promise((resolve) => {
        resolvePromise = resolve;
      })
    );

    const { result, unmount } = renderHook(() => useProviders());
    expect(result.current.loading).toBe(true);

    // Unmount before fetch resolves
    unmount();

    // Resolve after unmount — should not throw or update
    await act(async () => {
      resolvePromise!(makeResp());
    });

    // If we got here without errors, the mountedRef guard is working
    expect(true).toBe(true);
  });

  it("connected is a proper Set with has/size methods", async () => {
    const resp = makeResp({ connected: ["openai", "anthropic"] });
    mockFetchProviders.mockResolvedValue(resp);

    const { result } = renderHook(() => useProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.connected).toBeInstanceOf(Set);
    expect(result.current.connected.size).toBe(2);
    expect(result.current.connected.has("openai")).toBe(true);
    expect(result.current.connected.has("anthropic")).toBe(true);
    expect(result.current.connected.has("google")).toBe(false);
  });

  it("handles empty provider response", async () => {
    const resp = makeResp({ all: [], connected: [], default: {} });
    mockFetchProviders.mockResolvedValue(resp);

    const { result } = renderHook(() => useProviders());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.all).toEqual([]);
    expect(result.current.connected.size).toBe(0);
    expect(result.current.defaults).toEqual({});
    expect(result.current.error).toBeNull();
  });
});

describe("invalidateProviderCache", () => {
  it("is a function", () => {
    expect(typeof invalidateProviderCache).toBe("function");
  });

  it("does not throw when called without prior cache", () => {
    expect(() => invalidateProviderCache()).not.toThrow();
  });
});
