import { describe, expect, it } from "vitest";
import {
  appendCmdlineBlock, applyCmdlineHide, applyCmdlinePos, applyCmdlineShow, applyCmdlineSpecial,
  createCmdlineState, showCmdlineBlock,
} from "../state/cmdline";

describe("Neovim cmdline reducer", () => {
  it("expands repeated cells and records the show metadata", () => {
    const state = createCmdlineState();
    applyCmdlineShow(state, [[3, ":"], [4, "x", 2]], 4, ":", "> ", 2);

    expect(state.visible).toBe(true);
    expect(state.content.map((cell) => [cell.hlId, cell.text])).toEqual([
      [3, ":"], [4, "x"], [4, "x"],
    ]);
    expect(state.position).toBe(4);
    expect(state.firstChar).toBe(":");
    expect(state.prompt).toBe("> ");
    expect(state.indent).toBe(2);
  });

  it("updates the cursor and special-character state, then clears it on hide", () => {
    const state = createCmdlineState();
    applyCmdlinePos(state, -4);
    applyCmdlineSpecial(state, "?", true);
    expect(state.position).toBe(0);
    expect(state.specialChar).toBe("?");
    expect(state.specialShift).toBe(true);

    applyCmdlineHide(state);
    expect(state.visible).toBe(false);
    expect(state.specialChar).toBeNull();
    expect(state.specialShift).toBe(false);
  });

  it("appends block lines without flattening their line boundaries", () => {
    const state = createCmdlineState();
    appendCmdlineBlock(state, [
      [[1, "a", 2]],
      [[2, "b"]],
    ]);

    expect(state.block.map((line) => line.map((cell) => cell.text))).toEqual([["a", "a"], ["b"]]);
    expect(state.visible).toBe(false);
    showCmdlineBlock(state);
    expect(state.visible).toBe(true);
  });
});
