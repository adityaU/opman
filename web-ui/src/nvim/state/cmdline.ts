import type { CmdlineBlock, CmdlineState, UiCell } from "./types";

export type CmdlineModel = { -readonly [Key in keyof CmdlineState]: CmdlineState[Key] };

export function createCmdlineState(): CmdlineModel {
  return {
    content: [], position: 0, firstChar: "", prompt: "", indent: 0, specialChar: null,
    specialShift: false, visible: false, block: [],
  };
}

function cell(text: string, hlId: number): { readonly text: string; readonly hlId: number; readonly width: number } {
  return { text, hlId, width: 1 };
}

export function expandCells(cells: readonly UiCell[]): CmdlineModel["content"] {
  const result: Array<{ readonly text: string; readonly hlId: number; readonly width: number }> = [];
  for (const item of cells) {
    const repeat = Math.max(1, item[2] ?? 1);
    for (let index = 0; index < repeat; index += 1) result.push(cell(item[1], item[0]));
  }
  return result;
}

export function applyCmdlineShow(state: CmdlineModel, content: readonly UiCell[], position: number, firstChar: string, prompt: string, indent: number): void {
  state.content = expandCells(content);
  state.position = Math.max(0, position);
  state.firstChar = firstChar;
  state.prompt = prompt;
  state.indent = Math.max(0, indent);
  state.visible = true;
}

export function applyCmdlinePos(state: CmdlineModel, position: number): void {
  state.position = Math.max(0, position);
}

export function applyCmdlineSpecial(state: CmdlineModel, specialChar: string, shift: boolean): void {
  state.specialChar = specialChar;
  state.specialShift = shift;
}

export function applyCmdlineHide(state: CmdlineModel): void {
  state.visible = false;
  state.specialChar = null;
  state.specialShift = false;
}

export function appendCmdlineBlock(state: CmdlineModel, block: CmdlineBlock): void {
  const next = state.block.slice();
  for (const line of block) next.push(expandCells(line));
  state.block = next;
}

export function showCmdlineBlock(state: CmdlineModel): void {
  state.visible = true;
}
