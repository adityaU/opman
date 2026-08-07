/**
 * Unit tests for the useEscape hook. The global binding table it used to sit
 * beside now lives in the keymap matcher, which has its own suite.
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useEscape } from "../hooks/useKeyboard";

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
