import { describe, expect, it } from "vitest";
import { createTablineState, updateTabline } from "../state/tabline";
import type { TablineTab } from "../state/types";

const tabs: TablineTab[] = [
  { tab: "1", name: "main.ts", buffer: 12 },
  { tab: "2", name: "README", buffer: 13 },
];
const buffers = [
  { buffer: 12, name: "/workspace/src/main.ts" },
  { buffer: 13, name: "/workspace/README" },
];

describe("Neovim tabline reducer", () => {
  it("stores the current tab and complete tab metadata", () => {
    const state = createTablineState();
    updateTabline(state, 2, tabs, 12, buffers);

    expect(state).toEqual({ current: "2", tabs, currentTab: "2", currentBuffer: 12, buffers });
  });

  it("takes a snapshot of each update instead of retaining the caller array", () => {
    const state = createTablineState();
    updateTabline(state, 1, tabs, 12, buffers);
    tabs.pop();
    updateTabline(state, 1, [{ tab: "3", name: "scratch", buffer: 14 }], 14, [{ buffer: 14, name: "/tmp/scratch" }]);

    expect(state.tabs).toEqual([{ tab: "3", name: "scratch", buffer: 14 }]);
    expect(state.currentTab).toBe("1");
    expect(state.currentBuffer).toBe(14);
    expect(state.buffers).toEqual([{ buffer: 14, name: "/tmp/scratch" }]);
  });

  it("accepts older updates without a buffer list", () => {
    const state = createTablineState();
    updateTabline(state, 4, tabs, 12);

    expect(state.buffers).toEqual([]);
    expect(state.currentBuffer).toBe(12);
  });
});
