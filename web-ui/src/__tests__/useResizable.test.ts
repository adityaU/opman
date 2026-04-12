/**
 * Unit tests for useResizable hook.
 */
import { describe, it, expect, afterEach } from "vitest";
import { renderHook, act, fireEvent } from "@testing-library/react";
import { useResizable } from "../hooks/useResizable";

afterEach(() => {
  // Clean up body styles that may have been applied by the hook
  document.body.style.userSelect = "";
  document.body.style.cursor = "";
});

describe("useResizable", () => {
  it("initializes with the given size", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300 })
    );
    expect(result.current.size).toBe(300);
    expect(result.current.isDragging).toBe(false);
  });

  it("setSize updates size", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300 })
    );
    act(() => {
      result.current.setSize(400);
    });
    expect(result.current.size).toBe(400);
  });

  it("handleProps has correct className for horizontal", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300, direction: "horizontal" })
    );
    expect(result.current.handleProps.className).toContain("resize-handle-horizontal");
    expect(result.current.handleProps.style.cursor).toBe("col-resize");
  });

  it("handleProps has correct className for vertical", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 200, direction: "vertical" })
    );
    expect(result.current.handleProps.className).toContain("resize-handle-vertical");
    expect(result.current.handleProps.style.cursor).toBe("row-resize");
  });

  it("onMouseDown sets isDragging to true", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300 })
    );
    // Simulate the mousedown event
    act(() => {
      const fakeEvent = {
        preventDefault: () => {},
        clientX: 100,
        clientY: 100,
      } as unknown as React.MouseEvent;
      result.current.handleProps.onMouseDown(fakeEvent);
    });
    expect(result.current.isDragging).toBe(true);
  });

  it("dragging respects min and max bounds", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300, minSize: 200, maxSize: 500, direction: "horizontal" })
    );

    // Start drag at clientX = 300
    act(() => {
      const fakeEvent = {
        preventDefault: () => {},
        clientX: 300,
        clientY: 0,
      } as unknown as React.MouseEvent;
      result.current.handleProps.onMouseDown(fakeEvent);
    });
    expect(result.current.isDragging).toBe(true);

    // Move mouse to simulate drag beyond max (300 + 300 = 600 > max 500)
    act(() => {
      document.dispatchEvent(new MouseEvent("mousemove", { clientX: 600 }));
    });
    expect(result.current.size).toBe(500); // clamped to max

    // Move mouse to simulate drag below min (300 - 200 = 100 < min 200)
    act(() => {
      document.dispatchEvent(new MouseEvent("mousemove", { clientX: 100 }));
    });
    expect(result.current.size).toBe(200); // clamped to min

    // Mouse up ends drag
    act(() => {
      document.dispatchEvent(new MouseEvent("mouseup"));
    });
    expect(result.current.isDragging).toBe(false);
  });

  it("reverse mode: dragging left increases size", () => {
    const { result } = renderHook(() =>
      useResizable({ initialSize: 300, minSize: 100, maxSize: 600, direction: "horizontal", reverse: true })
    );

    act(() => {
      const fakeEvent = {
        preventDefault: () => {},
        clientX: 500,
        clientY: 0,
      } as unknown as React.MouseEvent;
      result.current.handleProps.onMouseDown(fakeEvent);
    });

    // Move left by 100 → size should increase from 300 to 400 (reverse: startSize - delta = 300 - (400-500) = 300 + 100 = 400)
    act(() => {
      document.dispatchEvent(new MouseEvent("mousemove", { clientX: 400 }));
    });
    expect(result.current.size).toBe(400);

    act(() => {
      document.dispatchEvent(new MouseEvent("mouseup"));
    });
  });
});
