/**
 * Unit tests for useToast hook.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useToast } from "../hooks/useToast";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useToast", () => {
  it("starts with empty toasts", () => {
    const { result } = renderHook(() => useToast());
    expect(result.current.toasts).toEqual([]);
  });

  it("addToast adds a toast and returns its id", () => {
    const { result } = renderHook(() => useToast());
    let toastId: number;
    act(() => {
      toastId = result.current.addToast("Hello", "info");
    });
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].message).toBe("Hello");
    expect(result.current.toasts[0].type).toBe("info");
    expect(result.current.toasts[0].id).toBe(toastId!);
  });

  it("default type is info", () => {
    const { result } = renderHook(() => useToast());
    act(() => {
      result.current.addToast("msg");
    });
    expect(result.current.toasts[0].type).toBe("info");
  });

  it("supports multiple toasts", () => {
    const { result } = renderHook(() => useToast());
    act(() => {
      result.current.addToast("one", "success");
      result.current.addToast("two", "error");
      result.current.addToast("three", "warning");
    });
    expect(result.current.toasts).toHaveLength(3);
    expect(result.current.toasts.map((t) => t.type)).toEqual(["success", "error", "warning"]);
  });

  it("removeToast removes a specific toast", () => {
    const { result } = renderHook(() => useToast());
    let id1: number, id2: number;
    act(() => {
      id1 = result.current.addToast("one");
      id2 = result.current.addToast("two");
    });
    act(() => {
      result.current.removeToast(id1!);
    });
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].id).toBe(id2!);
  });

  it("auto-removes toast after duration", () => {
    const { result } = renderHook(() => useToast());
    act(() => {
      result.current.addToast("auto", "info", 2000);
    });
    expect(result.current.toasts).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(result.current.toasts).toHaveLength(0);
  });

  it("default auto-remove duration is 3000ms", () => {
    const { result } = renderHook(() => useToast());
    act(() => {
      result.current.addToast("default duration");
    });
    expect(result.current.toasts).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(2999);
    });
    expect(result.current.toasts).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current.toasts).toHaveLength(0);
  });

  it("manual remove cancels auto-remove timer", () => {
    const { result } = renderHook(() => useToast());
    let id: number;
    act(() => {
      id = result.current.addToast("manual", "info", 5000);
    });
    act(() => {
      result.current.removeToast(id!);
    });
    expect(result.current.toasts).toHaveLength(0);
    // Should not cause any error when the timer fires
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(result.current.toasts).toHaveLength(0);
  });
});
