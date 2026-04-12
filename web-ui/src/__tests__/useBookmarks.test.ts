/**
 * Unit tests for useBookmarks hook.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useBookmarks } from "../hooks/useBookmarks";

// ── Mock localStorage ──────────────────────────────────
const store = new Map<string, string>();
const localStorageMock: Storage = {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => { store.set(key, value); },
  removeItem: (key: string) => { store.delete(key); },
  clear: () => store.clear(),
  get length() { return store.size; },
  key: (_i: number) => null,
};
Object.defineProperty(globalThis, "localStorage", { value: localStorageMock, writable: true });

beforeEach(() => {
  store.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useBookmarks", () => {
  it("starts with empty bookmarks", () => {
    const { result } = renderHook(() => useBookmarks());
    expect(result.current.allBookmarks).toEqual([]);
  });

  it("toggleBookmark adds a bookmark", () => {
    const { result } = renderHook(() => useBookmarks());
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "Hello world");
    });
    expect(result.current.allBookmarks).toHaveLength(1);
    expect(result.current.allBookmarks[0].messageId).toBe("msg1");
    expect(result.current.allBookmarks[0].sessionId).toBe("sess1");
    expect(result.current.allBookmarks[0].role).toBe("user");
    expect(result.current.allBookmarks[0].preview).toBe("Hello world");
  });

  it("toggleBookmark removes an existing bookmark", () => {
    const { result } = renderHook(() => useBookmarks());
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "Hello");
    });
    expect(result.current.allBookmarks).toHaveLength(1);
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "Hello");
    });
    expect(result.current.allBookmarks).toHaveLength(0);
  });

  it("isBookmarked returns correct state", () => {
    const { result } = renderHook(() => useBookmarks());
    expect(result.current.isBookmarked("msg1")).toBe(false);
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "Test");
    });
    expect(result.current.isBookmarked("msg1")).toBe(true);
  });

  it("removeBookmark removes a specific bookmark", () => {
    const { result } = renderHook(() => useBookmarks());
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "One");
      result.current.toggleBookmark("msg2", "sess1", "assistant", "Two");
    });
    expect(result.current.allBookmarks).toHaveLength(2);
    act(() => {
      result.current.removeBookmark("msg1");
    });
    expect(result.current.allBookmarks).toHaveLength(1);
    expect(result.current.allBookmarks[0].messageId).toBe("msg2");
  });

  it("getSessionBookmarks filters by session", () => {
    const { result } = renderHook(() => useBookmarks());
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "One");
      result.current.toggleBookmark("msg2", "sess2", "user", "Two");
      result.current.toggleBookmark("msg3", "sess1", "assistant", "Three");
    });
    const sess1 = result.current.getSessionBookmarks("sess1");
    expect(sess1).toHaveLength(2);
    expect(sess1.every((b) => b.sessionId === "sess1")).toBe(true);
    const sess2 = result.current.getSessionBookmarks("sess2");
    expect(sess2).toHaveLength(1);
  });

  it("truncates preview to 120 characters", () => {
    const { result } = renderHook(() => useBookmarks());
    const longText = "a".repeat(200);
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", longText);
    });
    expect(result.current.allBookmarks[0].preview).toHaveLength(120);
  });

  it("persists bookmarks to localStorage", () => {
    const { result } = renderHook(() => useBookmarks());
    act(() => {
      result.current.toggleBookmark("msg1", "sess1", "user", "Hello");
    });
    const stored = store.get("opman-bookmarks");
    expect(stored).toBeDefined();
    const parsed = JSON.parse(stored!);
    expect(parsed).toHaveLength(1);
    expect(parsed[0].messageId).toBe("msg1");
  });

  it("loads bookmarks from localStorage on mount", () => {
    const data = [
      { messageId: "msg1", sessionId: "sess1", role: "user", preview: "Test", createdAt: 1000 },
    ];
    store.set("opman-bookmarks", JSON.stringify(data));
    const { result } = renderHook(() => useBookmarks());
    expect(result.current.allBookmarks).toHaveLength(1);
    expect(result.current.allBookmarks[0].messageId).toBe("msg1");
  });

  it("sorts allBookmarks by most recent first", () => {
    const data = [
      { messageId: "msg1", sessionId: "sess1", role: "user", preview: "Old", createdAt: 1000 },
      { messageId: "msg2", sessionId: "sess1", role: "user", preview: "New", createdAt: 2000 },
    ];
    store.set("opman-bookmarks", JSON.stringify(data));
    const { result } = renderHook(() => useBookmarks());
    expect(result.current.allBookmarks[0].messageId).toBe("msg2");
    expect(result.current.allBookmarks[1].messageId).toBe("msg1");
  });
});
