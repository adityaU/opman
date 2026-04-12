/**
 * Unit tests for useKeyboard and useEscape hooks.
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useKeyboard, useEscape } from "../hooks/useKeyboard";

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Helper: dispatch a KeyboardEvent on document ───────
function fireKey(key: string, opts: Partial<KeyboardEvent> = {}) {
  const event = new KeyboardEvent("keydown", {
    key,
    bubbles: true,
    cancelable: true,
    ...opts,
  });
  document.dispatchEvent(event);
  return event;
}

// ═══════════════════════════════════════════════════════
// useEscape
// ═══════════════════════════════════════════════════════

describe("useEscape", () => {
  it("calls handler on Escape press", () => {
    const handler = vi.fn();
    renderHook(() => useEscape(handler));
    fireKey("Escape");
    expect(handler).toHaveBeenCalledOnce();
  });

  it("does not call handler on other keys", () => {
    const handler = vi.fn();
    renderHook(() => useEscape(handler));
    fireKey("Enter");
    fireKey("a");
    expect(handler).not.toHaveBeenCalled();
  });

  it("uses latest handler ref (no stale closure)", () => {
    const handler1 = vi.fn();
    const handler2 = vi.fn();
    const { rerender } = renderHook(({ fn }) => useEscape(fn), {
      initialProps: { fn: handler1 },
    });
    rerender({ fn: handler2 });
    fireKey("Escape");
    expect(handler1).not.toHaveBeenCalled();
    expect(handler2).toHaveBeenCalledOnce();
  });

  it("cleans up listener on unmount", () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() => useEscape(handler));
    unmount();
    fireKey("Escape");
    expect(handler).not.toHaveBeenCalled();
  });
});

// ═══════════════════════════════════════════════════════
// useKeyboard
// ═══════════════════════════════════════════════════════

describe("useKeyboard", () => {
  it("matches key + meta combo", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboard([{ key: "k", meta: true, handler }])
    );
    fireKey("k", { metaKey: true });
    expect(handler).toHaveBeenCalledOnce();
  });

  it("does not fire when meta is required but not pressed", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboard([{ key: "k", meta: true, handler }])
    );
    fireKey("k");
    expect(handler).not.toHaveBeenCalled();
  });

  it("matches shift modifier", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboard([{ key: "n", meta: true, shift: true, handler }])
    );
    fireKey("n", { metaKey: true, shiftKey: true });
    expect(handler).toHaveBeenCalledOnce();
  });

  it("does not match when shift is not pressed but required", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboard([{ key: "n", meta: true, shift: true, handler }])
    );
    fireKey("n", { metaKey: true });
    expect(handler).not.toHaveBeenCalled();
  });

  it("cleans up on unmount", () => {
    const handler = vi.fn();
    const { unmount } = renderHook(() =>
      useKeyboard([{ key: "b", meta: true, handler }])
    );
    unmount();
    fireKey("b", { metaKey: true });
    expect(handler).not.toHaveBeenCalled();
  });

  it("case-insensitive key matching", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboard([{ key: "B", meta: true, handler }])
    );
    fireKey("b", { metaKey: true });
    expect(handler).toHaveBeenCalledOnce();
  });
});
