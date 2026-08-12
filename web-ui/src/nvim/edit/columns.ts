/**
 * The one browser-side position boundary. Neovim's wire positions are
 * UTF-16 columns, exactly as the V1 server contract specifies. CodeMirror
 * offsets and JavaScript string indexes use the same units; byte/code-point
 * helpers stay here so tests can guard the boundary against Unicode drift.
 */

export function byteToUtf16(value: string, byte: number): number {
  const prefix = decodeUtf8Prefix(value, byte);
  return prefix === null ? -1 : prefix.length;
}

export function utf16ToByte(value: string, column: number): number {
  if (column < 0 || column > value.length) return -1;
  if (column > 0 && column < value.length) {
    const codeUnit = value.charCodeAt(column);
    if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) return -1;
    const previous = value.charCodeAt(column - 1);
    if (previous >= 0xd800 && previous <= 0xdbff) return -1;
  }
  return new TextEncoder().encode(value.slice(0, column)).length;
}

export function byteToCodePoint(value: string, byte: number): number {
  const prefix = decodeUtf8Prefix(value, byte);
  return prefix === null ? -1 : Array.from(prefix).length;
}

export function codePointToByte(value: string, character: number): number {
  if (character < 0) return -1;
  let points = 0;
  for (let index = 0; index < value.length;) {
    if (points === character) return new TextEncoder().encode(value.slice(0, index)).length;
    const codePoint = value.codePointAt(index);
    if (codePoint === undefined) return -1;
    index += codePoint > 0xffff ? 2 : 1;
    points += 1;
  }
  return points === character ? new TextEncoder().encode(value).length : -1;
}

export function nextCodePointOffset(value: string, offset: number): number {
  if (offset >= value.length) return offset;
  const codePoint = value.codePointAt(offset);
  return offset + (codePoint !== undefined && codePoint > 0xffff ? 2 : 1);
}

/** Advance over the grapheme at a CodeMirror position without allocating. */
export function nextGraphemeOffset(value: string, offset: number): number {
  let next = nextCodePointOffset(value, offset);
  while (next < value.length) {
    const codePoint = value.codePointAt(next);
    if (codePoint === undefined) return next;
    if (isCombining(codePoint) || isVariationSelector(codePoint)) {
      next = nextCodePointOffset(value, next);
      continue;
    }
    if (codePoint !== 0x200d) return next;
    next = nextCodePointOffset(value, next);
    if (next < value.length) next = nextCodePointOffset(value, next);
  }
  return next;
}

/** Convert the CodeMirror document offset to the V1 UTF-16 wire position. */
export function offsetToPosition(doc: Text, offset: number): TextPosition {
  const line = doc.lineAt(Math.max(0, Math.min(doc.length, offset)));
  return { line: line.number - 1, column: clampUtf16Column(line.text, offset - line.from) };
}

/** Convert a V1 UTF-16 wire position to a clamped CodeMirror document offset. */
export function positionToOffset(doc: Text, position: TextPosition): number {
  const line = doc.line(Math.max(1, Math.min(doc.lines, position.line + 1)));
  return line.from + clampUtf16Column(line.text, position.column);
}

function clampUtf16Column(value: string, column: number): number {
  const clamped = Math.max(0, Math.min(value.length, column));
  if (clamped > 0 && clamped < value.length) {
    const current = value.charCodeAt(clamped);
    const previous = value.charCodeAt(clamped - 1);
    if (current >= 0xdc00 && current <= 0xdfff && previous >= 0xd800 && previous <= 0xdbff) {
      return clamped - 1;
    }
  }
  return clamped;
}

function isCombining(codePoint: number): boolean {
  return (codePoint >= 0x300 && codePoint <= 0x36f)
    || (codePoint >= 0x1ab0 && codePoint <= 0x1aff)
    || (codePoint >= 0x1dc0 && codePoint <= 0x1dff)
    || (codePoint >= 0x20d0 && codePoint <= 0x20ff)
    || (codePoint >= 0xfe20 && codePoint <= 0xfe2f);
}

function isVariationSelector(codePoint: number): boolean {
  return (codePoint >= 0xfe00 && codePoint <= 0xfe0f)
    || (codePoint >= 0xe0100 && codePoint <= 0xe01ef);
}

function decodeUtf8Prefix(value: string, byte: number): string | null {
  // JS strings are UTF-16, so the browser cannot inspect arbitrary UTF-8
  // bytes without encoding once. A round-trip keeps this helper exact.
  const encoded = new TextEncoder().encode(value);
  if (byte < 0 || byte > encoded.length) return null;
  try {
    const prefix = new TextDecoder("utf-8", { fatal: true }).decode(encoded.slice(0, byte));
    return new TextEncoder().encode(prefix).length === byte ? prefix : null;
  } catch {
    return null;
  }
}
import type { Text } from "@codemirror/state";
import type { TextPosition } from "./wire";
