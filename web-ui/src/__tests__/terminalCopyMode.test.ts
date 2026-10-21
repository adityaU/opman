import { describe, expect, it } from "vitest";
import { copyModeStep, selectedRange, type CopyState } from "../terminal-panel/copyMode";

const g = { rows: 100, page: 20 };
const at = (cursor: number, anchor: number | null = null): CopyState => ({ cursor, anchor });

describe("copy mode", () => {
  it("moves a line with j and k and stops at the ends", () => {
    expect(copyModeStep(at(10), "j", false, g).state.cursor).toBe(11);
    expect(copyModeStep(at(10), "k", false, g).state.cursor).toBe(9);
    expect(copyModeStep(at(0), "k", false, g).state.cursor).toBe(0);
    expect(copyModeStep(at(99), "ArrowDown", false, g).state.cursor).toBe(99);
  });

  it("moves by half and whole pages", () => {
    expect(copyModeStep(at(50), "u", true, g).state.cursor).toBe(40);
    expect(copyModeStep(at(50), "d", true, g).state.cursor).toBe(60);
    expect(copyModeStep(at(50), "PageUp", false, g).state.cursor).toBe(30);
    expect(copyModeStep(at(50), "f", true, g).state.cursor).toBe(70);
  });

  it("jumps to the top and the bottom", () => {
    expect(copyModeStep(at(50), "g", false, g).state.cursor).toBe(0);
    expect(copyModeStep(at(50), "G", false, g).state.cursor).toBe(99);
  });

  it("starts and ends a selection with v, and selects the lines between", () => {
    const started = copyModeStep(at(10), "v", false, g).state;
    expect(started.anchor).toBe(10);
    const moved = copyModeStep(started, "j", false, g).state;
    expect(selectedRange(moved)).toEqual([10, 11]);
    expect(copyModeStep(moved, "v", false, g).state.anchor).toBeNull();
    expect(selectedRange(at(20, 25))).toEqual([20, 25]);
  });

  it("copies on y or Enter and leaves on q or Escape", () => {
    expect(copyModeStep(at(5), "y", false, g).effect).toBe("copy");
    expect(copyModeStep(at(5), "Enter", false, g).effect).toBe("copy");
    expect(copyModeStep(at(5), "q", false, g).effect).toBe("exit");
    expect(copyModeStep(at(5), "Escape", false, g).effect).toBe("exit");
  });

  it("ignores keys it does not know", () => {
    const step = copyModeStep(at(5), "x", false, g);
    expect(step).toEqual({ state: at(5), effect: "none" });
  });
});
