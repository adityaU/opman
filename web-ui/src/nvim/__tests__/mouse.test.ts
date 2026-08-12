import { describe, expect, it, vi } from "vitest";
import { createMouseHandlers, encodeMouseModifier, pointerToMouseMessage } from "../input/mouse";

const layout = { grid: 1, left: 10, top: 20, cellWidth: 10, cellHeight: 20, width: 80, height: 24 };
const pointer = (init: Partial<PointerEvent>): PointerEvent => ({
  clientX: 35, clientY: 65, button: 0, buttons: 1,
  shiftKey: false, ctrlKey: false, altKey: false, metaKey: false,
  preventDefault: vi.fn(), ...init,
} as unknown as PointerEvent);

describe("Neovim mouse input", () => {
  it("maps cells and modifiers to the wire tuple", () => {
    expect(pointerToMouseMessage(pointer({ shiftKey: true, ctrlKey: true }), layout, "press", "left"))
      .toEqual({ type: "input_mouse", button: "left", action: "press", modifier: "SC", grid: 1, row: 2, col: 2 });
    expect(encodeMouseModifier({ shiftKey: true, ctrlKey: true, altKey: true, metaKey: true })).toBe("SCAM");
  });

  it("coalesces drag moves to one animation-frame message", () => {
    const sent: unknown[] = [];
    let requested = 0;
    const handlers = createMouseHandlers({
      layout: () => layout,
      send: (message) => sent.push(message),
      requestFrame: () => {
        requested += 1;
        return 1;
      },
      cancelFrame: vi.fn(),
    });
    handlers.onPointerMove(pointer({ clientX: 20 }));
    handlers.onPointerMove(pointer({ clientX: 50 }));
    expect(sent).toHaveLength(0);
    expect(requested).toBe(1);
    handlers.flushDrag();
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({ action: "drag", col: 4 });
  });

  it("converts line and pixel wheel deltas", () => {
    const sent: unknown[] = [];
    const preventDefault = vi.fn();
    const handlers = createMouseHandlers({ layout: () => layout, send: (message) => sent.push(message), lineHeight: 10 });
    handlers.onWheel({ deltaX: 0, deltaY: 2, deltaMode: 1, clientX: 15, clientY: 25, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, preventDefault } as unknown as WheelEvent);
    handlers.onWheel({ deltaX: 0, deltaY: 20, deltaMode: 0, clientX: 15, clientY: 25, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, preventDefault } as unknown as WheelEvent);
    handlers.onWheel({ deltaX: 0, deltaY: 1, deltaMode: 2, clientX: 15, clientY: 25, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, preventDefault } as unknown as WheelEvent);
    expect(sent).toHaveLength(7);
    expect((sent[0] as { button: string }).button).toBe("wheel");
    expect((sent[0] as { action: string }).action).toBe("down");
    expect((sent[4] as { action: string }).action).toBe("down");
    expect(preventDefault).toHaveBeenCalledTimes(3);
  });

  it("uses the grid under the pointer for split-window clicks", () => {
    const sent: unknown[] = [];
    const handlers = createMouseHandlers({
      layout: () => layout,
      layoutForPoint: (clientX) => clientX > 50 ? { ...layout, grid: 2, left: 50 } : layout,
      send: (message) => sent.push(message),
    });
    handlers.onPointerDown(pointer({ clientX: 75, clientY: 25 }));
    expect(sent[0]).toMatchObject({ type: "input_mouse", grid: 2, col: 2 });
  });
});
