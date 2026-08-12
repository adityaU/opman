import type { PopupmenuItem, PopupmenuItemWire, PopupmenuState } from "./types";

export type PopupmenuModel = { -readonly [Key in keyof PopupmenuState]: PopupmenuState[Key] };

export function createPopupmenuState(): PopupmenuModel {
  return { items: [], selected: -1, row: 0, col: 0, visible: false };
}

function item(value: PopupmenuItemWire): PopupmenuItem {
  const text = (key: string): string => typeof value[key] === "string" ? value[key] as string : "";
  const flag = (key: string): boolean => value[key] === true;
  return {
    word: text("word"), abbr: text("abbr"), menu: text("menu"), info: text("info"), kind: text("kind"),
    icase: flag("icase"), dup: flag("dup"), empty: flag("empty"),
  };
}

export function showPopupmenu(state: PopupmenuModel, items: readonly PopupmenuItemWire[], selected: number, row: number, col: number): void {
  state.items = items.map(item);
  state.selected = selected;
  state.row = row;
  state.col = col;
  state.visible = true;
}

export function selectPopupmenu(state: PopupmenuModel, selected: number): void {
  state.selected = selected;
}

export function hidePopupmenu(state: PopupmenuModel): void {
  state.visible = false;
}
