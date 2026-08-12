import { describe, expect, it } from "vitest";
import { createPopupmenuState, hidePopupmenu, selectPopupmenu, showPopupmenu } from "../state/popupmenu";

describe("Neovim popupmenu reducer", () => {
  it("normalizes completion fields and records its position", () => {
    const state = createPopupmenuState();
    showPopupmenu(state, [{ word: "printf", abbr: "pri", menu: "[C]", info: "int", kind: "f", icase: true, dup: false, empty: true }], 0, 4, 6);

    expect(state.visible).toBe(true);
    expect(state.items).toEqual([{
      word: "printf", abbr: "pri", menu: "[C]", info: "int", kind: "f", icase: true, dup: false, empty: true,
    }]);
    expect(state).toMatchObject({ selected: 0, row: 4, col: 6 });
  });

  it("updates the selected item and hides without discarding candidates", () => {
    const state = createPopupmenuState();
    showPopupmenu(state, [{ word: "one" }, { word: "two" }], 1, 0, 0);
    selectPopupmenu(state, -1);
    hidePopupmenu(state);

    expect(state.visible).toBe(false);
    expect(state.selected).toBe(-1);
    expect(state.items.map((item) => item.word)).toEqual(["one", "two"]);
  });

  it("uses safe defaults for absent or non-string wire fields", () => {
    const state = createPopupmenuState();
    showPopupmenu(state, [{ word: 42, menu: null, icase: 1, dup: true }], 3, 1, 2);

    expect(state.items[0]).toEqual({
      word: "", abbr: "", menu: "", info: "", kind: "", icase: false, dup: true, empty: false,
    });
  });
});
