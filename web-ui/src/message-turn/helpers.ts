import type { ModelRef } from "./types";

/** Render a model reference as a display string. */
export function modelLabel(m: ModelRef): string {
  if (typeof m === "string") return m;
  return m.modelID || JSON.stringify(m);
}
