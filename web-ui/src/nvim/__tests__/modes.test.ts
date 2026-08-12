import { describe, expect, it } from "vitest";
import { applyModeChange, applyModeInfoSet, createModeState, currentMode, cursorInvertsCell } from "../state/modes";
import type { ModeInfo } from "../state/types";

const mode: ModeInfo = {
  name: "insert", shortName: "I", cursorShape: "vertical", cellPercentage: 25, attrId: 9,
  blinkWait: 700, blinkOn: 400, blinkOff: 250, conceal: "", canInsert: true, canUndo: true,
};

describe("Neovim mode reducer", () => {
  it("stores mode_info_set cursor and blink metadata without dropping fields", () => {
    const state = createModeState();
    applyModeInfoSet(state, [true, [mode]]);

    expect(state.enabled).toBe(true);
    expect(state.modes.get("insert")).toEqual(mode);
    expect(currentMode({ ...state, current: "insert" })).toEqual(mode);
  });

  it("updates the active mode and its cursor attribute on mode_change", () => {
    const state = createModeState();
    applyModeInfoSet(state, [true, [mode]]);
    applyModeChange(state, "insert", 9);

    expect(state.current).toBe("insert");
    expect(state.attrId).toBe(9);
    expect(cursorInvertsCell(state)).toBe(false);
  });

  it("treats mode_change attr_id zero as the terminal invert-cell cursor", () => {
    const state = createModeState();
    applyModeChange(state, "normal", 0);

    expect(cursorInvertsCell(state)).toBe(true);
    applyModeChange(state, "normal", 1);
    expect(cursorInvertsCell(state)).toBe(false);
  });
});
