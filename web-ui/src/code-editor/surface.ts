import type { EditorEngine } from "../editor-engine/preference";

/** Whether the CodeMirror surface should attach the selected Neovim editing engine. */
export function shouldAttachNeovim(
  layout: "desktop" | "mobile" | undefined,
  engine: EditorEngine,
  isMobile: boolean,
): boolean {
  if (engine !== "neovim") return false;
  if (layout === "mobile") return false;
  if (layout === "desktop") return true;
  return !isMobile;
}
