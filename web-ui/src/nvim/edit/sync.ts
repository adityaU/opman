import type { ChangeSpec } from "@codemirror/state";
import type { Text } from "@codemirror/state";
import type { ViewUpdate } from "@codemirror/view";
import { offsetToPosition } from "./columns";
import type { ClientMsg, ControlMsg } from "./wire";

export type EditPayload = Omit<Extract<ClientMsg, { type: "edit" }>, "type" | "changedtick" | "edit_id">;
export interface EditRecord { readonly editId: string; readonly payload: EditPayload; readonly baseTick: number; }

export interface LineChange {
  readonly first_line: number;
  readonly last_line: number;
  readonly lines: readonly string[];
}

/** Map Neovim's half-open line replacement to one CodeMirror change. */
export function lineChangeToSpec(doc: Text, change: LineChange): ChangeSpec | null {
  const lineCount = doc.lines;
  const { first_line: first, last_line: last, lines } = change;
  if (first < 0 || last < first || last > lineCount || first > lineCount) return null;

  const firstOffset = first === lineCount ? doc.length : doc.line(first + 1).from;
  const lastOffset = last === lineCount ? doc.length : doc.line(last + 1).from;
  let from = firstOffset;
  let to = lastOffset;
  let insert = lines.join("\n");

  // A final line has no following newline in CodeMirror. Removing it must
  // also remove the separator owned by the preceding line.
  if (lines.length === 0 && first < last && last === lineCount && first > 0) {
    from -= 1;
  }

  // Inserting/replacing before an existing line needs the separator after
  // the inserted lines. Appending needs the separator before them.
  if (last < lineCount && lines.length > 0) insert += "\n";
  if (first === last && first === lineCount && doc.length > 0) {
    insert = `\n${insert}`;
  }
  return { from, to, insert };
}

export function attachedText(lines: readonly string[]): string {
  return lines.join("\n");
}

/** Collapse a CodeMirror update into the single replaced range V1 sends. */
export function editFromUpdate(update: ViewUpdate): EditPayload | null {
  let start = -1;
  let end = -1;
  let insertedFrom = -1;
  let insertedTo = -1;
  update.changes.iterChanges((fromA, toA, fromB, toB) => {
    if (start < 0) {
      start = fromA;
      insertedFrom = fromB;
    }
    end = toA;
    insertedTo = toB;
  });
  if (start < 0 || end < 0 || insertedFrom < 0 || insertedTo < 0) return null;
  const text = update.state.doc.sliceString(insertedFrom, insertedTo);
  return {
    start: offsetToPosition(update.startState.doc, start),
    end: offsetToPosition(update.startState.doc, end),
    lines: text.length === 0 ? [] : text.split("\n"),
  };
}

export function isResyncControl(message: ControlMsg): boolean {
  return message.type === "resync_required" || message.type === "buffer_detached";
}
