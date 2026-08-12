import type { ModeInfo, ModeInfoSet } from "./types";

export interface ModeState {
  enabled: boolean;
  readonly modes: Map<string, ModeInfo>;
  current: string;
  attrId: number;
}

export function createModeState(): ModeState {
  return { enabled: false, modes: new Map(), current: "normal", attrId: 0 };
}

export function applyModeInfo(state: ModeState, enabled: boolean, modes: readonly ModeInfo[]): void {
  state.enabled = enabled;
  state.modes.clear();
  for (const mode of modes) state.modes.set(mode.name, mode);
}

export function applyModeChange(state: ModeState, name: string, attrId: number): void {
  state.current = name;
  state.attrId = attrId;
}

export function applyModeInfoSet(state: ModeState, value: ModeInfoSet): void {
  applyModeInfo(state, value[0], value[1]);
}

export function currentMode(state: ModeState): ModeInfo | undefined {
  return state.modes.get(state.current);
}

export function cursorInvertsCell(state: ModeState): boolean {
  return state.attrId === 0;
}
