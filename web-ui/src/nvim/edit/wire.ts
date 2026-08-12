/** Text-frame protocol for the CodeMirror/Neovim edit engine. */

export interface TextPosition {
  readonly line: number;
  readonly column: number;
}

export interface VisualSelection {
  readonly start: TextPosition;
  readonly end: TextPosition;
}

export type ModeShort =
  | "normal"
  | "insert"
  | "replace"
  | "visual"
  | "visual_line"
  | "visual_block"
  | "operator_pending"
  | "command";

export type ExCommand =
  | { readonly command: "write" }
  | { readonly command: "write_all" }
  | { readonly command: "quit" }
  | { readonly command: "force_quit" }
  | { readonly command: "buffer_delete" }
  | { readonly command: "no_highlight" }
  | { readonly command: "edit_reload" }
  | { readonly command: "undo" }
  | { readonly command: "redo" }
  | {
      readonly command: "substitute";
      readonly pattern: string;
      readonly replacement: string;
      readonly global: boolean;
      readonly ignore_case: boolean;
    }
  | {
      readonly command: "sort";
      readonly reverse: boolean;
      readonly numeric: boolean;
      readonly unique: boolean;
      readonly ignore_case: boolean;
    };

export type ClientMsg =
  | { readonly type: "attach"; readonly path: string }
  | {
      readonly type: "edit";
      readonly changedtick: number;
      readonly start: TextPosition;
      readonly end: TextPosition;
      readonly lines: readonly string[];
      readonly edit_id: string;
    }
  | { readonly type: "input"; readonly keys: string }
  | { readonly type: "cursor"; readonly position: TextPosition }
  | {
      readonly type: "input_mouse";
      readonly button: string;
      readonly action: string;
      readonly modifier: string;
      readonly grid: number;
      readonly row: number;
      readonly col: number;
    }
  | { readonly type: "resize"; readonly rows: number; readonly cols: number }
  | { readonly type: "paste"; readonly data: string }
  | { readonly type: "command"; readonly command: ExCommand };

export interface BufferEntry {
  readonly name: string;
  readonly modified: boolean;
  readonly current: boolean;
}

export interface NvimLayout {
  readonly tabpages: number;
  readonly windows: number;
  readonly buffers: readonly BufferEntry[];
}

export type ControlMsg =
  | { readonly type: "ready" }
  | {
      readonly type: "cmdline";
      readonly visible: boolean;
      readonly first_char: string;
      readonly content: string;
      readonly position: number;
    }
  | { readonly type: "search"; readonly pattern: string | null }
  | { readonly type: "action"; readonly name: string }
  | { readonly type: "layout"; readonly layout: NvimLayout }
  | { readonly type: "input_ack" }
  | {
      readonly type: "attached";
      readonly buffer: number;
      readonly path: string;
      readonly changedtick: number;
      readonly lines: readonly string[];
    }
  | {
      readonly type: "buffer_changed";
      readonly buffer: number;
      readonly changedtick: number;
      readonly first_line: number;
      readonly last_line: number;
      readonly new_last_line: number;
      readonly lines: readonly string[];
      readonly origin: string | null;
    }
  | {
      readonly type: "buffer_detached";
      readonly buffer: number;
      readonly changedtick: number;
      readonly reason: string;
    }
  | { readonly type: "resync_required"; readonly changedtick: number; readonly reason: string }
  | {
      readonly type: "state";
      readonly changedtick: number;
      readonly cursor: TextPosition;
      readonly mode: string;
      readonly mode_short: string;
      readonly visual: VisualSelection | null;
    }
  | { readonly type: "command_output"; readonly changedtick: number; readonly output: string }
  | { readonly type: "message"; readonly changedtick: number; readonly kind: string; readonly text: string }
  | { readonly type: "error"; readonly message: string }
  | { readonly type: "exited"; readonly code: number | null }
  | { readonly type: "superseded" }
  | { readonly type: "too_slow" };

type RecordValue = Record<string, unknown>;

function record(value: unknown): RecordValue | null {
  return typeof value === "object" && value !== null ? value as RecordValue : null;
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function position(value: unknown): TextPosition | null {
  const item = record(value);
  const line = numberValue(item?.line);
  const column = numberValue(item?.column);
  return line !== null && column !== null ? { line, column } : null;
}

function lines(value: unknown): readonly string[] | null {
  return Array.isArray(value) && value.every((line) => typeof line === "string")
    ? value as string[]
    : null;
}

function bufferEntry(value: unknown): BufferEntry | null {
  const item = record(value);
  const name = stringValue(item?.name);
  return name !== null && typeof item?.modified === "boolean" && typeof item?.current === "boolean"
    ? { name, modified: item.modified, current: item.current }
    : null;
}

function layout(value: unknown): NvimLayout | null {
  const item = record(value);
  const tabpages = numberValue(item?.tabpages);
  const windows = numberValue(item?.windows);
  if (tabpages === null || windows === null || !Array.isArray(item?.buffers)) return null;
  const buffers = item.buffers.map(bufferEntry);
  return buffers.every((entry): entry is BufferEntry => entry !== null)
    ? { tabpages, windows, buffers }
    : null;
}

function visual(value: unknown): VisualSelection | null {
  const item = record(value);
  const start = position(item?.start);
  const end = position(item?.end);
  return start && end ? { start, end } : null;
}

/** Parse only the closed control variants emitted by V1. */
export function parseControl(value: unknown): ControlMsg | null {
  const item = record(value);
  const type = stringValue(item?.type);
  if (!type) return null;
  if (type === "ready" || type === "input_ack" || type === "superseded" || type === "too_slow") return { type };
  if (type === "error") {
    const message = stringValue(item?.message);
    return message === null ? null : { type, message };
  }
  if (type === "exited") {
    const code = item?.code;
    return code === null || code === undefined || typeof code === "number" ? { type, code: code ?? null } : null;
  }
  if (type === "attached") {
    const buffer = numberValue(item?.buffer);
    const path = stringValue(item?.path);
    const changedtick = numberValue(item?.changedtick);
    const valueLines = lines(item?.lines);
    return buffer !== null && path !== null && changedtick !== null && valueLines !== null
      ? { type, buffer, path, changedtick, lines: valueLines }
      : null;
  }
  if (type === "buffer_changed") {
    const buffer = numberValue(item?.buffer);
    const changedtick = numberValue(item?.changedtick);
    const firstLine = numberValue(item?.first_line);
    const lastLine = numberValue(item?.last_line);
    const newLastLine = numberValue(item?.new_last_line);
    const valueLines = lines(item?.lines);
    const origin = item?.origin === null || item?.origin === undefined ? null : stringValue(item?.origin);
    return buffer !== null && changedtick !== null && firstLine !== null && lastLine !== null
      && newLastLine !== null && valueLines !== null && (item?.origin === null || item?.origin === undefined || origin !== null)
      ? { type, buffer, changedtick, first_line: firstLine, last_line: lastLine, new_last_line: newLastLine, lines: valueLines, origin }
      : null;
  }
  if (type === "buffer_detached") {
    const buffer = numberValue(item?.buffer);
    const changedtick = numberValue(item?.changedtick);
    const reason = stringValue(item?.reason);
    return buffer !== null && changedtick !== null && reason !== null ? { type, buffer, changedtick, reason } : null;
  }
  if (type === "resync_required") {
    const changedtick = numberValue(item?.changedtick);
    const reason = stringValue(item?.reason);
    return changedtick !== null && reason !== null ? { type, changedtick, reason } : null;
  }
  if (type === "state") {
    const changedtick = numberValue(item?.changedtick);
    const cursor = position(item?.cursor);
    const mode = stringValue(item?.mode);
    const modeShort = stringValue(item?.mode_short);
    const selected = item?.visual === null || item?.visual === undefined ? null : visual(item?.visual);
    return changedtick !== null && cursor !== null && mode !== null && modeShort !== null
      && (item?.visual === null || item?.visual === undefined || selected !== null)
      ? { type, changedtick, cursor, mode, mode_short: modeShort, visual: selected }
      : null;
  }
  if (type === "cmdline") {
    const content = stringValue(item?.content);
    const firstChar = stringValue(item?.first_char);
    const position = numberValue(item?.position);
    return typeof item?.visible === "boolean" && content !== null && firstChar !== null && position !== null
      ? { type, visible: item.visible, first_char: firstChar, content, position }
      : null;
  }
  if (type === "search") {
    const pattern = item?.pattern === null || item?.pattern === undefined ? null : stringValue(item.pattern);
    return item?.pattern === null || item?.pattern === undefined || pattern !== null ? { type, pattern } : null;
  }
  if (type === "action") {
    const name = stringValue(item?.name);
    return name === null ? null : { type, name };
  }
  if (type === "layout") {
    const parsed = layout(item?.layout);
    return parsed === null ? null : { type, layout: parsed };
  }
  if (type === "command_output" || type === "message") {
    const changedtick = numberValue(item?.changedtick);
    const output = type === "command_output" ? stringValue(item?.output) : null;
    const kind = type === "message" ? stringValue(item?.kind) : null;
    const text = type === "message" ? stringValue(item?.text) : null;
    if (changedtick === null) return null;
    if (type === "command_output") return output === null ? null : { type, changedtick, output };
    return kind === null || text === null ? null : { type, changedtick, kind, text };
  }
  return null;
}
