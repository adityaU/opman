/** The sidebar preference is total: anything unreadable falls back, width is clamped. */
import { describe, it, expect } from "vitest";
import {
  DEFAULT_SIDEBAR_PREFS,
  loadSidebarPrefs,
  persistSidebarPrefs,
  SIDEBAR_MAX_WIDTH,
  SIDEBAR_MIN_WIDTH,
} from "../utils/sidebarPrefs";

function store(value?: string): Pick<Storage, "getItem" | "setItem"> {
  let held = value;
  return {
    getItem: () => held ?? null,
    setItem: (_key: string, next: string) => {
      held = next;
    },
  };
}

const stored = (prefs: unknown) => store(JSON.stringify(prefs));

describe("sidebar prefs", () => {
  it("falls back when nothing is stored", () => {
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, store())).toEqual(DEFAULT_SIDEBAR_PREFS);
  });

  it("round-trips a closed sidebar", () => {
    const storage = store();
    persistSidebarPrefs({ open: false, width: 320 }, storage);
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, storage)).toEqual({ open: false, width: 320 });
  });

  it("keeps the fallback for fields it cannot read", () => {
    const prefs = loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, stored({ open: "yes", width: null }));
    expect(prefs).toEqual(DEFAULT_SIDEBAR_PREFS);
  });

  it("clamps a width outside the drag bounds", () => {
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, stored({ open: true, width: 9000 })).width).toBe(
      SIDEBAR_MAX_WIDTH,
    );
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, stored({ open: true, width: 1 })).width).toBe(
      SIDEBAR_MIN_WIDTH,
    );
  });

  it("survives malformed JSON and non-objects", () => {
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, store("{not json"))).toEqual(
      DEFAULT_SIDEBAR_PREFS,
    );
    expect(loadSidebarPrefs(DEFAULT_SIDEBAR_PREFS, stored(42))).toEqual(DEFAULT_SIDEBAR_PREFS);
  });
});
