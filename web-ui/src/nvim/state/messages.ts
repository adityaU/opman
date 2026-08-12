import type { MessageHistoryEntry, MessageItem, MessageState, MsgKind, UiCell } from "./types";
import { expandCells } from "./cmdline";

export type { MessageItem } from "./types";

export type MessageModel = { -readonly [Key in keyof MessageState]: MessageState[Key] };

export function createMessageState(): MessageModel {
  return { items: [], mode: [], command: [], ruler: [], history: [], position: null, scrolled: false, separator: "" };
}

function item(kind: MsgKind | string, content: readonly UiCell[], replaceLast: boolean, history: boolean): MessageItem {
  return { kind, content: expandCells(content), replaceLast, history };
}

export function applyMessageShow(state: MessageModel, kind: MsgKind | string, content: readonly UiCell[], replaceLast: boolean, inHistory: boolean): void {
  const next = item(kind, content, replaceLast, inHistory);
  const items = state.items.slice();
  if (replaceLast && items.length > 0) items[items.length - 1] = next;
  else items.push(next);
  state.items = items;
  if (inHistory) state.history = state.history.concat(next);
}

export function clearMessages(state: MessageModel): void {
  state.items = [];
}

export function setMessagePosition(state: MessageModel, position: number, scrolled: boolean, separator: string): void {
  state.position = position;
  state.scrolled = scrolled;
  state.separator = separator;
}

export function setMessageMode(state: MessageModel, content: readonly UiCell[]): void {
  state.mode = expandCells(content);
}

export function setMessageCommand(state: MessageModel, content: readonly UiCell[]): void {
  state.command = expandCells(content);
}

export function setMessageRuler(state: MessageModel, content: readonly UiCell[]): void {
  state.ruler = expandCells(content);
}

export function showMessageHistory(state: MessageModel, entries: readonly MessageHistoryEntry[]): void {
  const history = entries.map(([kind, content]) => item(kind, content, false, true));
  state.history = history;
  state.items = history.slice();
}
