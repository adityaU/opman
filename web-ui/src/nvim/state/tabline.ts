import type { TablineBuffer, TablineState, TablineTab } from "./types";

export type TablineModel = { -readonly [Key in keyof TablineState]: TablineState[Key] };

export function createTablineState(): TablineModel {
  return { current: "", tabs: [], currentTab: "", currentBuffer: 0, buffers: [] };
}

export function updateTabline(
  state: TablineModel,
  current: number,
  tabs: readonly TablineTab[],
  currentBuffer: number,
  buffers: readonly TablineBuffer[] = [],
): void {
  state.current = String(current);
  state.tabs = tabs.slice();
  state.currentTab = String(current);
  state.currentBuffer = currentBuffer;
  state.buffers = buffers.slice();
}
