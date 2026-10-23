import type { CSSProperties } from "react";
import { TERMINAL_CONTROL } from "./bashOutputFormat";

export interface AnsiChunk { text: string; style: CSSProperties }
const PALETTE = [
  "var(--color-text-muted)", "var(--color-error)", "var(--color-success)", "var(--color-warning)",
  "var(--color-secondary)", "var(--color-accent)", "var(--color-info)", "var(--color-text)",
];

function indexedColour(index: number): string | undefined {
  if (!Number.isInteger(index) || index < 0 || index > 255) return undefined;
  if (index < 16) return PALETTE[index % 8];
  if (index >= 232) {
    const level = 8 + (index - 232) * 10;
    return `rgb(${level}, ${level}, ${level})`;
  }
  const cube = index - 16;
  const level = (n: number) => n === 0 ? 0 : 55 + n * 40;
  return `rgb(${level(Math.floor(cube / 36))}, ${level(Math.floor(cube / 6) % 6)}, ${level(cube % 6)})`;
}

/** Parse terminal styles as React data. Output is never treated as HTML. */
export function ansiChunks(output: string): AnsiChunk[] {
  const chunks: AnsiChunk[] = [];
  let style: CSSProperties = {};
  let position = 0;
  for (const match of output.matchAll(TERMINAL_CONTROL)) {
    const index = match.index ?? 0;
    if (index > position) chunks.push({ text: output.slice(position, index), style });
    position = index + match[0].length;
    if (!/^\x1b\[[\d;]*m$/.test(match[0])) continue;
    const codes = match[0].slice(2, -1).split(";").map(Number);
    style = { ...style };
    for (let i = 0; i < codes.length; i++) {
      const code = codes[i];
      if (code === 0) { style = {}; continue; }
      if (code === 1) style.fontWeight = 700;
      else if (code === 2) style.opacity = 0.7;
      else if (code === 3) style.fontStyle = "italic";
      else if (code === 4) style.textDecoration = "underline";
      else if (code === 22) { delete style.fontWeight; delete style.opacity; }
      else if (code === 23) delete style.fontStyle;
      else if (code === 24) delete style.textDecoration;
      else if (code === 39) delete style.color;
      else if (code === 49) delete style.backgroundColor;
      else if (code >= 30 && code <= 37) style.color = PALETTE[code - 30];
      else if (code >= 90 && code <= 97) style.color = PALETTE[code - 90];
      else if (code >= 40 && code <= 47) style.backgroundColor = PALETTE[code - 40];
      else if (code >= 100 && code <= 107) style.backgroundColor = PALETTE[code - 100];
      else if (code === 38 || code === 48) {
        let colour: string | undefined;
        if (codes[i + 1] === 5) { colour = indexedColour(codes[i + 2]); i += 2; }
        else if (codes[i + 1] === 2) {
          const rgb = codes.slice(i + 2, i + 5);
          if (rgb.length === 3 && rgb.every(n => Number.isInteger(n) && n >= 0 && n <= 255)) {
            colour = `rgb(${rgb.join(", ")})`;
          }
          i += 4;
        }
        if (colour) style[code === 38 ? "color" : "backgroundColor"] = colour;
      }
    }
  }
  if (position < output.length) chunks.push({ text: output.slice(position), style });
  return chunks;
}
